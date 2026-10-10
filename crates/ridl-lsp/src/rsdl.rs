//! Hover, go-to-definition, find-references and rename in an `.rsdl` file
//! (rsdl reference §4).
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
//!
//! Find-references reports every rsdl reference the checker binds to the same
//! declaration, from [`ridl_sem::rsdl::references`]; on a declared interface
//! it adds the references the ridl navigation finds. Rename rewrites a system,
//! a component or a declared instance, with every rsdl reference and import
//! line that names it. An interface is renamed by the ridl rename, which
//! already rewrites the rsdl references to it; a service is not renamed.

use ridl_core::db::InputFile;
use ridl_core::package::{Package, Workspace, package_of, service_catalog};
use ridl_ir::v2;
use ridl_sem::docs::DocInfo;
use ridl_sem::rsdl::{
    CheckedSystem, DeclAttrs, InterfaceId, MemberRef, Target, UNIT_INSTANCE, reference_at,
};
use ridl_sem::{Symbol, SymbolKind, check_package, check_system, resolve_package};
use ridl_syntax::SyntaxKind;
use ridl_syntax::ast::AstNode;
use ridl_syntax::keywords;
use rowan::{TextRange, TextSize};

use crate::hover::{self, DocParts, HoverInfo};
use crate::nav;
use crate::rename::{self, Edit, RenameError};

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
    declaration_site(db, ws, std, &system, &found.target)
}

/// The declaration site of `target`, a target of `system`.
fn declaration_site(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    system: &CheckedSystem,
    target: &Target,
) -> Option<(InputFile, TextRange)> {
    let site = match target {
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

/// What the cursor at `offset` in `file` names: the target of the rsdl
/// reference there, or the system, component or declared instance whose own
/// name is there. The range is the part of the reference, or the name, under
/// the cursor.
fn target_at(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    system: &CheckedSystem,
    file: InputFile,
    offset: TextSize,
) -> Option<(Target, TextRange)> {
    if let Some(found) = reference_at(db, ws, std, system, file, offset) {
        return Some((found.target, found.range));
    }
    let at =
        |site: &ridl_sem::rsdl::Site| site.file == file && site.range.contains_inclusive(offset);
    if let Some(index) = system.systems.iter().position(|d| at(&d.name.site)) {
        return Some((Target::System(index), system.systems[index].name.site.range));
    }
    if let Some(index) = system.components.iter().position(|d| at(&d.name.site)) {
        return Some((
            Target::Component(index),
            system.components[index].name.site.range,
        ));
    }
    system
        .components
        .iter()
        .enumerate()
        .find_map(|(component, decl)| {
            let instances = decl.instances.as_ref()?;
            let instance = instances.iter().position(|named| at(&named.site))?;
            Some((
                Target::Instance {
                    component,
                    instance,
                },
                instances[instance].site.range,
            ))
        })
}

/// Whether two targets name the same declaration. A service and the inline
/// shape of that service are one declaration: both are written as the
/// service's dotted name.
fn same_declaration(a: &Target, b: &Target) -> bool {
    let service = |target: &Target| match target {
        Target::Service(name) | Target::Interface(InterfaceId::Inline { service: name }) => {
            Some(name.clone())
        }
        _ => None,
    };
    match (service(a), service(b)) {
        (Some(a), Some(b)) => a == b,
        (None, None) => a == b,
        _ => false,
    }
}

/// What find-references reports for a cursor in an `.rsdl` file.
pub enum References {
    /// A declared interface: the typl symbol, whose references the ridl
    /// navigation finds, and the rsdl references to it.
    Interface {
        symbol: Symbol,
        references: Vec<(InputFile, TextRange)>,
    },
    /// Any other rsdl target: its declaration site, when it has one, and the
    /// part of every rsdl reference that names it.
    Rsdl {
        declaration: Option<(InputFile, TextRange)>,
        references: Vec<(InputFile, TextRange)>,
    },
}

/// The references to what the cursor at `offset` in `file` names, or `None`
/// when the cursor names no rsdl target.
pub fn references(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    file: InputFile,
    offset: TextSize,
) -> Option<References> {
    let system = check_system(db, ws, std);
    let (target, _) = target_at(db, ws, std, &system, file, offset)?;
    let references = rsdl_references(db, ws, std, &system, &target);
    if let Target::Interface(InterfaceId::Declared { package, name }) = &target {
        let symbol = interface_symbol(db, ws, std, package, name)?;
        return Some(References::Interface { symbol, references });
    }
    Some(References::Rsdl {
        declaration: declaration_site(db, ws, std, &system, &target),
        references,
    })
}

/// The part of every rsdl reference of `system` that names `target`.
fn rsdl_references(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    system: &CheckedSystem,
    target: &Target,
) -> Vec<(InputFile, TextRange)> {
    ridl_sem::rsdl::references(db, ws, std, system)
        .into_iter()
        .filter(|found| same_declaration(&found.target, target))
        .map(|found| (found.reference.site.file, found.range))
        .collect()
}

/// What rename does at a cursor in an `.rsdl` file.
pub enum RenameAt {
    /// A system, component or declared instance, which [`rename`] rewrites;
    /// `span` is the name under the cursor.
    Rsdl { target: Target, span: TextRange },
    /// A service. Its dotted name is declared in ridl and is global to the
    /// workspace, so it is not renamed.
    Refused,
    /// A declared interface, or no rsdl target: the typl rename handles the
    /// cursor, as it handles an interface reference or a doc link in a ridl
    /// file.
    Typl,
}

/// What rename does at `offset` in `file`.
pub fn rename_at(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    file: InputFile,
    offset: TextSize,
) -> RenameAt {
    let system = check_system(db, ws, std);
    let Some((target, range)) = target_at(db, ws, std, &system, file, offset) else {
        return RenameAt::Typl;
    };
    match target {
        Target::Interface(InterfaceId::Declared { .. }) => RenameAt::Typl,
        Target::Service(_) | Target::Interface(InterfaceId::Inline { .. }) => RenameAt::Refused,
        Target::System(_) | Target::Component(_) | Target::Instance { .. } => {
            match last_segment(db, file, range) {
                Some(span) => RenameAt::Rsdl { target, span },
                None => RenameAt::Refused,
            }
        }
    }
}

/// The last name segment written inside `range` in `file`: `Cruise` in
/// `veh.topology.Cruise`.
fn last_segment(db: &dyn salsa::Database, file: InputFile, range: TextRange) -> Option<TextRange> {
    nav::source_file(db, file)
        .syntax()
        .covering_element(range)
        .into_node()
        .map_or_else(
            || Some(range),
            |node| {
                node.descendants_with_tokens()
                    .filter_map(|element| element.into_token())
                    .filter(|token| {
                        !token.kind().is_trivia()
                            && token.kind() != SyntaxKind::Dot
                            && range.contains_range(token.text_range())
                    })
                    .map(|token| token.text_range())
                    .last()
            },
        )
}

/// The edits renaming `target`, a system, component or declared instance, to
/// `new_name`: its declared name, the last segment of every rsdl reference to
/// it that is written with its name, and, for a system or a component, the
/// last segment of every import line that binds it.
///
/// A system or component name stays UpperCamelCase and an instance name
/// lowerCamelCase (rsdl reference §4); a new name that a declaration of an
/// affected package holds, or that another instance of the component holds,
/// is a collision.
pub fn rename(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    packages: &[Package],
    target: &Target,
    new_name: &str,
) -> Result<Vec<Edit>, RenameError> {
    let system = check_system(db, ws, std);
    let (old, package) = match target {
        Target::System(index) => {
            let decl = &system.systems[*index];
            (&decl.name, Some(decl.package.as_str()))
        }
        Target::Component(index) => {
            let decl = &system.components[*index];
            (&decl.name, Some(decl.package.as_str()))
        }
        Target::Instance {
            component,
            instance,
        } => {
            let instances = system.components[*component]
                .instances
                .as_ref()
                .ok_or(RenameError::NotRenameable)?;
            (&instances[*instance], None)
        }
        Target::Service(_) | Target::Interface(_) => return Err(RenameError::NotRenameable),
    };
    if new_name == old.name {
        return Ok(Vec::new());
    }
    if keywords::is_reserved(new_name) {
        return Err(RenameError::Reserved(new_name.to_string()));
    }
    match package {
        None => {
            if !is_lower_camel(new_name) {
                return Err(RenameError::CaseConvention(format!(
                    "`{new_name}` is not lowerCamelCase — an instance name must be"
                )));
            }
            if let Target::Instance { component, .. } = target
                && system.components[*component]
                    .instances
                    .iter()
                    .flatten()
                    .any(|named| named.name == new_name)
            {
                return Err(RenameError::Collision(new_name.to_string()));
            }
        }
        Some(package) => {
            if !rename::is_camel_case(new_name) {
                return Err(RenameError::CaseConvention(format!(
                    "`{new_name}` is not UpperCamelCase — a system or component name must be"
                )));
            }
            if package_declares(db, ws, std, &system, packages, package, new_name) {
                return Err(RenameError::Collision(new_name.to_string()));
            }
        }
    }

    let mut edits = vec![Edit {
        file: old.site.file,
        range: old.site.range,
    }];
    // A reference written through an import alias does not spell the old
    // name; it stays as written, as a ridl rename leaves it.
    for (file, range) in rsdl_references(db, ws, std, &system, target) {
        let Some(segment) = last_segment(db, file, range) else {
            continue;
        };
        if file
            .text(db)
            .get(usize::from(segment.start())..usize::from(segment.end()))
            == Some(old.name.as_str())
        {
            edits.push(Edit {
                file,
                range: segment,
            });
        }
    }
    if let Some(package) = package {
        let mut imported: Vec<String> = package.split('.').map(str::to_string).collect();
        imported.push(old.name.clone());
        for &importing in packages {
            let mut imports_target = false;
            for &file in importing.files(db) {
                for import in nav::source_file(db, file).imports() {
                    let Some(qualified) = import.qualified_name() else {
                        continue;
                    };
                    if nav::qualified_segments(qualified.syntax()) != imported {
                        continue;
                    }
                    imports_target = true;
                    if let Some(token) = nav::last_segment_token(&qualified) {
                        edits.push(Edit {
                            file,
                            range: token.text_range(),
                        });
                    }
                }
            }
            let importing_name = importing.name(db);
            if imports_target
                && importing_name != package
                && package_declares(db, ws, std, &system, packages, importing_name, new_name)
            {
                return Err(RenameError::Collision(new_name.to_string()));
            }
        }
    }
    Ok(rename::dedup(edits))
}

/// Whether the package `package` declares `name`: a typl, ridl or rsdl
/// declaration, which share the package's names (typl §3, TYPL-009).
fn package_declares(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    system: &CheckedSystem,
    packages: &[Package],
    package: &str,
    name: &str,
) -> bool {
    let rsdl = system
        .systems
        .iter()
        .map(|d| (&d.package, &d.name))
        .chain(system.components.iter().map(|d| (&d.package, &d.name)))
        .chain(system.distributions.iter().map(|d| (&d.package, &d.name)))
        .chain(system.deployments.iter().map(|d| (&d.package, &d.name)))
        .any(|(declaring, named)| declaring == package && named.name == name);
    rsdl || rename::package_named(db, packages, package)
        .is_some_and(|declaring| rename::declares(db, ws, std, declaring, name))
}

/// Whether `name` is lowerCamelCase: a leading ASCII lowercase letter and only
/// ASCII alphanumerics after.
fn is_lower_camel(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase()) && chars.all(|c| c.is_ascii_alphanumeric())
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
