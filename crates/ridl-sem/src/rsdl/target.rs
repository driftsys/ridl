//! What a reference in an `.rsdl` file names (rsdl reference §4), for the
//! language server. The reference is read from the checked model, and its
//! target from the checker's own results: an `offers` or `requires` line from
//! the component's resolved lines, a member line and a `for` reference through
//! the lookups the checker resolves them with. The language server does not
//! resolve a name itself, so it cannot navigate to a declaration the checker
//! would not bind.

use ridl_core::db::InputFile;
use ridl_core::package::{Package, Workspace, service_catalog};
use ridl_syntax::SyntaxKind;
use ridl_syntax::ast::AstNode;
use rowan::{TextRange, TextSize};

use super::closure::{InterfaceId, Lookup};
use super::{CheckedSystem, Reference, ReferenceForm};
use crate::resolve::source_file;

/// The declaration a reference names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A `system`, as an index into [`CheckedSystem::systems`].
    System(usize),
    /// A `component`, as an index into [`CheckedSystem::components`].
    Component(usize),
    /// One declared instance: the component, as an index into
    /// [`CheckedSystem::components`], and the instance, as an index into that
    /// component's `instances` list.
    Instance { component: usize, instance: usize },
    /// A `service`, by its dotted name.
    Service(String),
    /// A declared `interface`, or the inline shape of a service (rsdl §3.2).
    Interface(InterfaceId),
}

/// A reference and the declaration a part of it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceTarget {
    /// The reference as the model holds it.
    pub reference: Reference,
    /// The part of the reference that names the target: the instance segment
    /// of `Name.inst` for the instance, the segments before it for the
    /// component, and the whole reference for every other form.
    pub range: TextRange,
    pub target: Target,
}

/// Where a reference is written. Its role depends on the slot (rsdl §4).
#[derive(Clone, Copy)]
enum Slot<'a> {
    /// A member line of a `system`, `distribution` or `machine` in `package`;
    /// `placement` for a machine's, the one member line that may name an
    /// instance (rsdl §4).
    Member { package: &'a str, placement: bool },
    /// The `line`-th `offers` line of the `component`-th component.
    Offers { component: usize, line: usize },
    /// The `line`-th `requires` line of the `component`-th component.
    Requires { component: usize, line: usize },
    /// The `for` reference of a deployment in `package`.
    For { package: &'a str },
}

/// The reference of `system` written at `offset` in `file`, and what it names,
/// or `None` when no reference is there or the checker bound it to nothing.
///
/// `system` is the result of [`check_system`](super::check_system) for `ws`
/// and `std`.
pub fn reference_at(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    system: &CheckedSystem,
    file: InputFile,
    offset: TextSize,
) -> Option<ReferenceTarget> {
    let (reference, slot) = written_references(system)
        .into_iter()
        .find(|(reference, _)| {
            reference.site.file == file && reference.site.range.contains_inclusive(offset)
        })?;
    let catalog = service_catalog(db, ws, std);
    let lookup = Lookup::new(db, ws, std, system, &catalog);
    target_of(db, system, &lookup, reference, slot, offset)
}

/// Every reference of `system` the checker binds, with what it names. An
/// instance reference `Name.inst` appears twice: once for the component its segments before `inst` name, and once
/// for the instance, when the component declares it.
///
/// `system` is the result of [`check_system`](super::check_system) for `ws`
/// and `std`.
pub fn references(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    system: &CheckedSystem,
) -> Vec<ReferenceTarget> {
    let catalog = service_catalog(db, ws, std);
    let lookup = Lookup::new(db, ws, std, system, &catalog);
    let mut found = Vec::new();
    for (reference, slot) in written_references(system) {
        let range = reference.site.range;
        // The start of a reference is on its first part, its end on its
        // last part: the instance segment of an instance reference.
        let mut offsets = vec![range.start()];
        if matches!(reference.form, ReferenceForm::Instance { .. }) {
            offsets.push(range.end());
        }
        for offset in offsets {
            if let Some(target) = target_of(db, system, &lookup, reference, slot, offset) {
                found.push(target);
            }
        }
    }
    found
}

/// What `reference`, written in `slot`, names at `offset`, which lies inside
/// the reference.
fn target_of(
    db: &dyn salsa::Database,
    system: &CheckedSystem,
    lookup: &Lookup<'_>,
    reference: &Reference,
    slot: Slot<'_>,
    offset: TextSize,
) -> Option<ReferenceTarget> {
    let reference = reference.clone();
    let whole = reference.site.range;
    let (target, range) = match slot {
        Slot::Offers { component, line } => {
            let service = system.component_lines.get(component)?.offers.get(line)?;
            (Target::Service(service.clone()?), whole)
        }
        Slot::Requires { component, line } => {
            let interface = system.component_lines.get(component)?.requires.get(line)?;
            (Target::Interface(interface.clone()?), whole)
        }
        Slot::For { package } => {
            let ReferenceForm::Declared {
                package: written,
                name,
            } = &reference.form
            else {
                return None;
            };
            let index = lookup.system(package, written.as_deref(), name)?;
            (Target::System(index), whole)
        }
        Slot::Member { package, placement } => {
            match &reference.form {
                ReferenceForm::Declared {
                    package: written,
                    name,
                } => {
                    let index = lookup.component(package, written.as_deref(), name)?;
                    (Target::Component(index), whole)
                }
                // A `system` or `distribution` line naming an instance is
                // RSDL-602 or RSDL-903.
                ReferenceForm::Instance { .. } if !placement => return None,
                ReferenceForm::Instance {
                    package: written,
                    component,
                    instance,
                } => {
                    let index = lookup.component(package, written.as_deref(), component)?;
                    let (component_part, instance_part) = instance_ranges(db, &reference)?;
                    if instance_part.contains_inclusive(offset) {
                        let position = system.components[index]
                            .instances
                            .as_ref()?
                            .iter()
                            .position(|declared| declared.name == *instance)?;
                        let target = Target::Instance {
                            component: index,
                            instance: position,
                        };
                        (target, instance_part)
                    } else {
                        (Target::Component(index), component_part)
                    }
                }
                // A service a declared component offers does not stand for a
                // component (RSDL-504); the checker's `Lookup::record_offers`
                // reads the same resolved `offers` lines.
                ReferenceForm::Service { name }
                    if lookup.catalog.entries.contains_key(name)
                        && !system.component_lines.iter().any(|lines| {
                            lines.offers.iter().flatten().any(|offered| offered == name)
                        }) =>
                {
                    (Target::Service(name.clone()), whole)
                }
                ReferenceForm::Service { .. } | ReferenceForm::Unreadable => return None,
            }
        }
    };
    Some(ReferenceTarget {
        reference,
        range,
        target,
    })
}

/// Every reference of the model with its slot. References never overlap, so
/// the order does not decide which one a cursor is on.
fn written_references(system: &CheckedSystem) -> Vec<(&Reference, Slot<'_>)> {
    let mut references = Vec::new();
    let members = system
        .systems
        .iter()
        .map(|decl| (&decl.package, &decl.members, false))
        .chain(
            system
                .distributions
                .iter()
                .map(|decl| (&decl.package, &decl.members, false)),
        )
        .chain(system.deployments.iter().flat_map(|decl| {
            decl.machines
                .iter()
                .map(move |machine| (&decl.package, &machine.members, true))
        }));
    for (package, lines, placement) in members {
        for line in lines {
            let slot = Slot::Member { package, placement };
            references.push((&line.reference, slot));
        }
    }
    for (component, decl) in system.components.iter().enumerate() {
        for (line, member) in decl.offers.iter().enumerate() {
            references.push((&member.reference, Slot::Offers { component, line }));
        }
        for (line, member) in decl.requires.iter().enumerate() {
            references.push((&member.reference, Slot::Requires { component, line }));
        }
    }
    for decl in &system.deployments {
        if let Some(reference) = &decl.system {
            let package = &decl.package;
            references.push((reference, Slot::For { package }));
        }
    }
    references
}

/// The two parts of an instance reference `pkg.Name.inst`: from its start to
/// the end of `Name`, and the `inst` segment.
fn instance_ranges(
    db: &dyn salsa::Database,
    reference: &Reference,
) -> Option<(TextRange, TextRange)> {
    let root = source_file(db, reference.site.file);
    let node = root
        .syntax()
        .covering_element(reference.site.range)
        .into_node()?;
    let segments: Vec<TextRange> = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia() && token.kind() != SyntaxKind::Dot)
        .map(|token| token.text_range())
        .collect();
    let [.., name, instance] = segments.as_slice() else {
        return None;
    };
    Some((
        TextRange::new(reference.site.range.start(), name.end()),
        *instance,
    ))
}
