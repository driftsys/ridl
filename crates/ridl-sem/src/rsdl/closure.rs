//! The closure (rsdl reference §3.1): the `system`, the lines of every
//! component (§3.2), instances (§7) and the implicit component (§6).
//!
//! **Which rules run where.** The rules of one declaration run on every
//! declared component, whether the system lists it or not, and in a workspace
//! with no `system`: RSDL-309 to RSDL-312 on its lines, RSDL-306 and RSDL-307
//! on its `instances`. rsdl §3.1 draws no diagnostic for a component the
//! closure does not list and none for a workspace with no `system`; that is a
//! rule about the absence, and the §3.2 and §7 rules carry no closure
//! qualifier. The completeness checks quantify over the closure (§3.1 names
//! those of §8, §9 and §3.3), so they run only when a closure exists.

use std::collections::{BTreeMap, HashMap, HashSet};

use ridl_core::diag::DiagCode;
use ridl_core::package::{CatalogEntry, Package, ServiceCatalog, Workspace, package_of};

use super::{
    CheckedSystem, ComponentDecl, MemberRef, Reference, ReferenceForm, Reporter, Site,
    UNIT_INSTANCE,
};
use crate::resolve::{
    Resolution, SymbolKind, declared_symbols, qualified_segments, resolve_package, source_file,
};

/// A closure component's identity (rsdl §6): a declared component by its
/// package and name, or the implicit component of a lone service, named by the
/// service's dotted name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ComponentId {
    Declared { package: String, name: String },
    Implicit { service: String },
}

impl ComponentId {
    /// The name a diagnostic prints: the declared name, or the service's
    /// dotted name.
    pub fn text(&self) -> &str {
        match self {
            Self::Declared { name, .. } => name,
            Self::Implicit { service } => service,
        }
    }
}

/// An interface's identity (rsdl §3.2, §8): a declared `interface` by its
/// package and name, or the inline shape of a service, named by the service's
/// dotted name (ridl §14.5). The two stay apart because a one-segment service
/// name may be spelled like an interface.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InterfaceId {
    Declared { package: String, name: String },
    Inline { service: String },
}

impl InterfaceId {
    /// `pkg.Name` for a declared interface; the service's dotted name for an
    /// inline shape.
    pub fn text(&self) -> String {
        match self {
            Self::Declared { package, name } => format!("{package}.{name}"),
            Self::Inline { service } => service.clone(),
        }
    }
}

/// The resolved lines of one declared component, parallel to its `offers` and
/// `requires` lines (rsdl §3.2). An entry is `None` when its line drew
/// RSDL-307, RSDL-309, RSDL-310, RSDL-311 or RSDL-312.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComponentLines {
    /// The dotted name of the service each `offers` line names.
    pub offers: Vec<Option<String>>,
    /// The interface each `requires` line names.
    pub requires: Vec<Option<InterfaceId>>,
}

/// The closure of the workspace's system (rsdl §3.1) and what it reaches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Closure {
    /// The closure components, in the order the system lists them.
    pub components: Vec<ClosureComponent>,
    /// The closure services — every service a closure component offers — by
    /// dotted name.
    pub services: BTreeMap<String, ClosureService>,
    /// The closure interfaces — every interface a closure service lists —
    /// each with the closure services that list it, in service name order.
    /// More than one owner is RSDL-408.
    pub interface_owners: BTreeMap<InterfaceId, Vec<String>>,
}

/// One component of the closure (rsdl §3.1, §6, §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureComponent {
    pub id: ComponentId,
    /// An index into `CheckedSystem::components`; `None` for an implicit
    /// component.
    pub decl: Option<usize>,
    /// The instance names (rsdl §7): the declared ones, less a written `Unit`
    /// and a repeat, or the unit instance `Unit` when none is declared.
    pub instances: Vec<String>,
    /// The `external` flag; an implicit component is never external (§6).
    pub external: bool,
    /// The services it offers, in line order.
    pub offers: Vec<String>,
    /// Where a diagnostic about the component points: the declared name, or
    /// the system member line of an implicit component.
    pub site: Site,
}

/// One service of the closure (rsdl §8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureService {
    /// The package that declares the service.
    pub package: String,
    /// The closure components that offer it, as indexes into
    /// `Closure::components`, in closure order. More than one is RSDL-502.
    pub offerers: Vec<usize>,
    /// The interfaces it lists, in slot order; one `Inline` for an inline
    /// shape.
    pub listed_interfaces: Vec<InterfaceId>,
}

/// What a member line of a `system`, `distribution` or `machine` names (rsdl
/// §4, §6), before the slot checks it against the closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MemberTarget {
    /// A declared component, as an index into `CheckedSystem::components`.
    Component(usize),
    /// One instance of a declared component, the instance name as written.
    Instance(usize, String),
    /// A lone service that no declared component offers: its implicit
    /// component (rsdl §6).
    Implicit(String),
    /// Nothing the slot can name. The slot reports its unknown-name code.
    Unknown,
}

/// The name lookups the rsdl passes share (rsdl §4): the declared components
/// by package and name, the imports of each package, the package resolutions
/// that bind an interface name, the service catalog, and the declared offerer
/// of each service.
pub(super) struct Lookup<'a> {
    db: &'a dyn salsa::Database,
    ws: Workspace,
    pub(super) catalog: &'a ServiceCatalog,
    components: HashMap<(String, String), usize>,
    /// Per package: each local name an import binds, to the imported package
    /// path and name. Imports bind package-wide (ADR-0002 §2).
    imports: HashMap<String, HashMap<String, (String, String)>>,
    /// Per package: its resolved names, which bind a bare interface name.
    resolutions: HashMap<String, Resolution>,
    /// The first declared component, in or out of the closure, with an
    /// `offers` line naming each service (rsdl §6). Filled by
    /// [`Lookup::record_offers`].
    declared_offerer: HashMap<String, usize>,
}

impl<'a> Lookup<'a> {
    /// Indexes the declarations of `system`; the first declaration of a name
    /// in a package wins.
    pub(super) fn new(
        db: &'a dyn salsa::Database,
        ws: Workspace,
        std: Package,
        system: &CheckedSystem,
        catalog: &'a ServiceCatalog,
    ) -> Self {
        let mut components = HashMap::new();
        for (index, decl) in system.components.iter().enumerate() {
            components
                .entry((decl.package.clone(), decl.name.name.clone()))
                .or_insert(index);
        }
        let packages: HashSet<&String> = system
            .systems
            .iter()
            .map(|decl| &decl.package)
            .chain(system.components.iter().map(|decl| &decl.package))
            .chain(system.distributions.iter().map(|decl| &decl.package))
            .chain(system.deployments.iter().map(|decl| &decl.package))
            .collect();
        let mut imports = HashMap::new();
        let mut resolutions = HashMap::new();
        for name in packages {
            let Some(package) = package_of(db, ws, name.clone()) else {
                continue;
            };
            imports.insert(name.clone(), package_imports(db, package));
            resolutions.insert(name.clone(), resolve_package(db, ws, package, std));
        }
        Self {
            db,
            ws,
            catalog,
            components,
            imports,
            resolutions,
            declared_offerer: HashMap::new(),
        }
    }

    /// Records the declared offerer of every service from the resolved lines.
    pub(super) fn record_offers(&mut self, lines: &[ComponentLines]) {
        for (index, component) in lines.iter().enumerate() {
            for service in component.offers.iter().flatten() {
                self.declared_offerer
                    .entry(service.clone())
                    .or_insert(index);
            }
        }
    }

    /// The declared component a `Name` or `pkg.Name` reference written in
    /// package `from` names, as an index into `CheckedSystem::components`.
    pub(super) fn component(&self, from: &str, package: Option<&str>, name: &str) -> Option<usize> {
        self.declared(&self.components, from, package, name)
    }

    /// rsdl §4 and typl §3.2: a qualified name binds the declaration of that
    /// package; a bare name binds a declaration of `from`, then an import of
    /// `from`.
    fn declared(
        &self,
        index: &HashMap<(String, String), usize>,
        from: &str,
        package: Option<&str>,
        name: &str,
    ) -> Option<usize> {
        let key = match package {
            Some(package) => (package.to_string(), name.to_string()),
            None => {
                let local = (from.to_string(), name.to_string());
                if index.contains_key(&local) {
                    local
                } else {
                    self.imports.get(from)?.get(name)?.clone()
                }
            }
        };
        index.get(&key).copied()
    }

    /// The interface a `Name` or `pkg.Name` reference written in package `from`
    /// names: a bare name through the package's resolved names (its own
    /// declarations and its imports), a qualified name in that package, not
    /// `internal` unless declared in `from` (typl §3.3).
    pub(super) fn interface(
        &self,
        from: &str,
        package: Option<&str>,
        name: &str,
    ) -> Option<InterfaceId> {
        let symbol = match package {
            None => self.resolutions.get(from)?.symbols.get(name)?.clone(),
            Some(package) => {
                let target = package_of(self.db, self.ws, package.to_string())?;
                let symbol = declared_symbols(self.db, target).get(name)?.clone();
                if symbol.internal && symbol.package != from {
                    return None;
                }
                symbol
            }
        };
        (symbol.kind == SymbolKind::Interface).then_some(InterfaceId::Declared {
            package: symbol.package,
            name: symbol.name,
        })
    }
}

/// The interfaces a catalog service lists (rsdl §8 step 1): its inline shape,
/// or each named shape of its list. A shape reference the catalog keeps bare
/// names an interface of the service's own package.
pub(super) fn service_interfaces(service: &str, entry: &CatalogEntry) -> Vec<InterfaceId> {
    if entry.inline {
        return vec![InterfaceId::Inline {
            service: service.to_string(),
        }];
    }
    entry
        .interface_refs
        .iter()
        .map(|reference| match reference.rsplit_once('.') {
            Some((package, name)) => InterfaceId::Declared {
                package: package.to_string(),
                name: name.to_string(),
            },
            None => InterfaceId::Declared {
                package: entry.package.clone(),
                name: reference.clone(),
            },
        })
        .collect()
}

/// Every import of `package`, as local name to package path and name; among
/// imports binding one local name, the first wins, as in the resolver.
fn package_imports(
    db: &dyn salsa::Database,
    package: Package,
) -> HashMap<String, (String, String)> {
    let mut imports = HashMap::new();
    for file in package.files(db) {
        for import in source_file(db, *file).imports() {
            let Some(qualified) = import.qualified_name() else {
                continue;
            };
            let mut segments = qualified_segments(&qualified);
            let name = segments.pop().unwrap_or_default();
            if segments.is_empty() || name.is_empty() || segments.iter().any(String::is_empty) {
                continue;
            }
            let local = import
                .alias()
                .and_then(|alias| alias.ident_token())
                .map_or_else(|| name.clone(), |token| token.text().to_string());
            imports.entry(local).or_insert((segments.join("."), name));
        }
    }
    imports
}

/// Resolves the lines of every declared component and checks its `instances`
/// (rsdl §3.2, §7).
pub(super) fn component_lines(
    lookup: &Lookup,
    system: &CheckedSystem,
    reporter: &mut Reporter,
) -> Vec<ComponentLines> {
    system
        .components
        .iter()
        .map(|component| {
            check_instances(component, reporter);
            let mut services = HashSet::new();
            let mut offers = Vec::new();
            for line in &component.offers {
                let service = offered_service(lookup, line, reporter).filter(|service| {
                    let first = services.insert(service.clone());
                    if !first {
                        reporter.error(
                            DiagCode::RSDL_309,
                            line.reference.site,
                            format!(
                                "`{}` offers `{service}` twice — one `offers` line per service \
                                 (rsdl reference §3.2)",
                                component.name.name
                            ),
                        );
                    }
                    first
                });
                offers.push(service);
            }
            let mut interfaces = HashSet::new();
            let mut requires = Vec::new();
            for line in &component.requires {
                let interface = required_interface(lookup, &component.package, line, reporter)
                    .filter(|interface| {
                        let first = interfaces.insert(interface.clone());
                        if !first {
                            reporter.error(
                                DiagCode::RSDL_309,
                                line.reference.site,
                                format!(
                                    "`{}` requires `{}` twice — one `requires` line per interface \
                                     (rsdl reference §3.2)",
                                    component.name.name,
                                    interface.text()
                                ),
                            );
                        }
                        first
                    });
                requires.push(interface);
            }
            ComponentLines { offers, requires }
        })
        .collect()
}

/// The service an `offers` line names (rsdl §3.2, §4): a `pkg.service`
/// reference to a service the catalog holds. Anything else is RSDL-310.
fn offered_service(lookup: &Lookup, line: &MemberRef, reporter: &mut Reporter) -> Option<String> {
    let reference = &line.reference;
    if written_unit(reference, reporter) {
        return None;
    }
    if let ReferenceForm::Service { name } = &reference.form
        && lookup.catalog.entries.contains_key(name)
    {
        return Some(name.clone());
    }
    reporter.error(
        DiagCode::RSDL_310,
        reference.site,
        format!(
            "`{}` is not a service — `offers` names one service by its dotted name, such as \
             `veh.adas.cruise` (rsdl reference §3.2)",
            reference.text()
        ),
    );
    None
}

/// The interface a `requires` line names (rsdl §3.2, §4): `Name` or `pkg.Name`
/// resolving to an interface, or `pkg.service` naming a service with an inline
/// shape. A service with a list of shapes is RSDL-311; anything else is
/// RSDL-312.
fn required_interface(
    lookup: &Lookup,
    package: &str,
    line: &MemberRef,
    reporter: &mut Reporter,
) -> Option<InterfaceId> {
    let reference = &line.reference;
    if written_unit(reference, reporter) {
        return None;
    }
    match &reference.form {
        ReferenceForm::Declared {
            package: written,
            name,
        } => {
            if let Some(interface) = lookup.interface(package, written.as_deref(), name) {
                return Some(interface);
            }
        }
        ReferenceForm::Service { name } => {
            if let Some(entry) = lookup.catalog.entries.get(name) {
                if entry.inline {
                    return Some(InterfaceId::Inline {
                        service: name.clone(),
                    });
                }
                let listed: Vec<String> = service_interfaces(name, entry)
                    .iter()
                    .map(|interface| format!("`{}`", interface.text()))
                    .collect();
                reporter.error(
                    DiagCode::RSDL_311,
                    reference.site,
                    format!(
                        "`{name}` is a service with a list of interfaces, and `requires` names one \
                         interface — it lists {} (rsdl reference §3.2)",
                        if listed.is_empty() {
                            "no interface".to_string()
                        } else {
                            listed.join(", ")
                        }
                    ),
                );
                return None;
            }
        }
        ReferenceForm::Instance { .. } | ReferenceForm::Unreadable => {}
    }
    reporter.error(
        DiagCode::RSDL_312,
        reference.site,
        format!(
            "`{}` is neither an interface nor a service with an inline shape — `requires` names \
             one interface (rsdl reference §3.2)",
            reference.text()
        ),
    );
    None
}

/// RSDL-307 (rsdl §7): `Unit` written as the instance segment of a reference.
/// Returns whether it was, so the caller drops the line.
pub(super) fn written_unit(reference: &Reference, reporter: &mut Reporter) -> bool {
    let ReferenceForm::Instance {
        component,
        instance,
        ..
    } = &reference.form
    else {
        return false;
    };
    if instance != UNIT_INSTANCE {
        return false;
    }
    reporter.error(
        DiagCode::RSDL_307,
        reference.site,
        format!(
            "`{component}.Unit` names the unit instance, which is never written — write \
             `{component}` for the component's one instance (rsdl reference §7)"
        ),
    );
    true
}

/// RSDL-307 and RSDL-306 on a component's `instances` list (rsdl §7).
fn check_instances(component: &ComponentDecl, reporter: &mut Reporter) {
    let Some(instances) = &component.instances else {
        return;
    };
    let mut seen = HashSet::new();
    for instance in instances {
        if instance.name == UNIT_INSTANCE {
            reporter.error(
                DiagCode::RSDL_307,
                instance.site,
                "`Unit` is the name of the unit instance, which is never written — name each \
                 instance for its role, such as `primary` (rsdl reference §7)"
                    .to_string(),
            );
        } else if !seen.insert(instance.name.as_str()) {
            reporter.error(
                DiagCode::RSDL_306,
                instance.site,
                format!(
                    "`{}` declares the instance `{}` twice (rsdl reference §7)",
                    component.name.name, instance.name
                ),
            );
        }
    }
}

/// A declared component's instance names (rsdl §7): its declared instances
/// less a written `Unit` and a repeat, or the unit instance when it declares
/// none.
fn instance_names(component: &ComponentDecl) -> Vec<String> {
    let Some(instances) = &component.instances else {
        return vec![UNIT_INSTANCE.to_string()];
    };
    let mut names: Vec<String> = Vec::new();
    for instance in instances {
        if instance.name != UNIT_INSTANCE && !names.contains(&instance.name) {
            names.push(instance.name.clone());
        }
    }
    names
}

/// Reads what a member line names (rsdl §4, §6), from a declaration of
/// package `from`. Reports RSDL-307 for a written `Unit` and RSDL-504 for a
/// service a declared component offers, and returns `None` after either.
pub(super) fn member_target(
    lookup: &Lookup,
    system: &CheckedSystem,
    from: &str,
    reference: &Reference,
    reporter: &mut Reporter,
) -> Option<MemberTarget> {
    if written_unit(reference, reporter) {
        return None;
    }
    Some(match &reference.form {
        ReferenceForm::Declared { package, name } => lookup
            .component(from, package.as_deref(), name)
            .map_or(MemberTarget::Unknown, MemberTarget::Component),
        ReferenceForm::Instance {
            package,
            component,
            instance,
        } => lookup
            .component(from, package.as_deref(), component)
            .map_or(MemberTarget::Unknown, |decl| {
                MemberTarget::Instance(decl, instance.clone())
            }),
        ReferenceForm::Service { name } if lookup.catalog.entries.contains_key(name) => {
            if let Some(&offerer) = lookup.declared_offerer.get(name) {
                let offerer = &system.components[offerer].name.name;
                reporter.error(
                    DiagCode::RSDL_504,
                    reference.site,
                    format!(
                        "`{name}` is offered by the component `{offerer}`, so the service does not \
                         stand for a component — write `{offerer}` (rsdl reference §6)"
                    ),
                );
                return None;
            }
            MemberTarget::Implicit(name.clone())
        }
        ReferenceForm::Service { .. } | ReferenceForm::Unreadable => MemberTarget::Unknown,
    })
}

/// Reads the closure from the first `system` (rsdl §3.1), or `None` when the
/// workspace declares none. A later `system` is RSDL-601 and is not read.
pub(super) fn closure(
    lookup: &Lookup,
    system: &CheckedSystem,
    lines: &[ComponentLines],
    reporter: &mut Reporter,
) -> Option<Closure> {
    let (first, later) = system.systems.split_first()?;
    for second in later {
        reporter.error(
            DiagCode::RSDL_601,
            second.name.site,
            format!(
                "`{}` is a second `system`: a workspace declares at most one, and `{}` is \
                 declared (rsdl reference §3.1)",
                second.name.name, first.name.name
            ),
        );
    }
    let mut closure = Closure::default();
    for line in &first.members {
        let reference = &line.reference;
        let Some(target) = member_target(lookup, system, &first.package, reference, reporter)
        else {
            continue;
        };
        let component = match target {
            MemberTarget::Component(decl) => {
                let component = &system.components[decl];
                ClosureComponent {
                    id: ComponentId::Declared {
                        package: component.package.clone(),
                        name: component.name.name.clone(),
                    },
                    decl: Some(decl),
                    instances: instance_names(component),
                    external: component.external,
                    offers: lines[decl].offers.iter().flatten().cloned().collect(),
                    site: component.name.site,
                }
            }
            MemberTarget::Implicit(service) => ClosureComponent {
                id: ComponentId::Implicit {
                    service: service.clone(),
                },
                decl: None,
                instances: vec![UNIT_INSTANCE.to_string()],
                external: false,
                offers: vec![service],
                site: reference.site,
            },
            MemberTarget::Instance(..) | MemberTarget::Unknown => {
                reporter.error(
                    DiagCode::RSDL_602,
                    reference.site,
                    format!(
                        "`{}` is not a component or a service — a `system` lists components and \
                         lone services (rsdl reference §3.1)",
                        reference.text()
                    ),
                );
                continue;
            }
        };
        if closure
            .components
            .iter()
            .any(|listed| listed.id == component.id)
        {
            reporter.error(
                DiagCode::RSDL_603,
                reference.site,
                format!(
                    "`{}` is listed twice in `{}` (rsdl reference §3.1)",
                    component.id.text(),
                    first.name.name
                ),
            );
            continue;
        }
        closure.components.push(component);
    }
    for (index, component) in closure.components.iter().enumerate() {
        for service in &component.offers {
            let entry = &lookup.catalog.entries[service];
            closure
                .services
                .entry(service.clone())
                .or_insert_with(|| ClosureService {
                    package: entry.package.clone(),
                    offerers: Vec::new(),
                    listed_interfaces: service_interfaces(service, entry),
                })
                .offerers
                .push(index);
        }
    }
    for (name, service) in &closure.services {
        for interface in &service.listed_interfaces {
            let owners = closure
                .interface_owners
                .entry(interface.clone())
                .or_default();
            if !owners.contains(name) {
                owners.push(name.clone());
            }
        }
    }
    Some(closure)
}
