//! The rsdl checker (rsdl reference v0.2): one workspace-level query,
//! [`check_system`], over every `.rsdl` file of the workspace. The closure is
//! workspace-wide (rsdl §3.1), so the rsdl checks are one query over the whole
//! workspace rather than an extension of the per-package `check_package`.
//!
//! The query collects the five declarations into the model below, reads each
//! reference by case (rsdl §4) and each attribute block against the rsdl-owned
//! keys (rsdl §5), and returns the model with its diagnostics. The model is
//! what the later passes and the lowering read: every entry carries the
//! [`Site`] it is written at, so a pass reports against the source and the
//! language server navigates to it.
//!
//! The diagnostics carry a [`FileId`] indexing the workspace's files in
//! package-then-file order — the order `ridl_core::package::service_catalog`
//! uses — so a driver remaps both with one list of render ids.

mod attrs;
mod closure;
mod collect;
mod distribution;
mod doc_links;
mod lower;
mod placement;
mod resolve;
mod target;

pub use closure::{
    Closure, ClosureComponent, ClosureService, ComponentId, ComponentLines, InterfaceId,
};
pub use distribution::{DistributionDependency, DistributionFacts};
pub use lower::lower_system;
pub use placement::{DeploymentPlacement, Placement};
pub use resolve::ResolvedRequire;
pub use target::{ReferenceTarget, Target, reference_at};

use std::collections::{BTreeSet, HashMap};

use ridl_core::db::{InputFile, profile_of_path};
use ridl_core::diag::{DiagCode, Diagnostic, FileId, Severity, SourceMap, Span};
use ridl_core::package::{Package, Workspace, service_catalog};
use ridl_ir::codegen::depth::ceil_ratio;
use ridl_ir::v2;
use ridl_syntax::Profile;
use rowan::TextRange;

use crate::docs::DocInfo;
use crate::resolve::source_file;

/// The name of the unit instance: the one instance of a component that
/// declares no `instances` (rsdl §7). It is never written in source; a written
/// `Unit` is kept in the model so that its diagnostic (RSDL-307) can report it.
pub const UNIT_INSTANCE: &str = "Unit";

/// Checks every `.rsdl` file of `ws` and returns the collected model with its
/// diagnostics (rsdl reference v0.2).
///
/// `std` is the embedded `ridl.std` package. The package resolver needs it to
/// bind a bare interface name in a `requires` line; it declares no rsdl.
#[salsa::tracked(returns(clone))]
pub fn check_system(db: &dyn salsa::Database, ws: Workspace, std: Package) -> CheckedSystem {
    let mut reporter = Reporter::new(db, ws);
    let mut system = collect::collect(db, ws, &mut reporter);
    lint_docs(db, ws, &mut reporter);
    doc_links::resolve(db, ws, std, &mut system, &mut reporter);
    collect::check_declaration_names(db, ws, &system, &mut reporter);
    let catalog = service_catalog(db, ws, std);
    let mut lookup = closure::Lookup::new(db, ws, std, &system, &catalog);
    let lines = closure::component_lines(&lookup, &system, &mut reporter);
    lookup.record_offers(&lines);
    system.closure =
        closure::closure(&lookup, &system, &lines, &mut reporter).map(|mut closure| {
            closure.requires = resolve::resolve(&lookup, &system, &lines, &closure, &mut reporter);
            closure.distribution_facts =
                distribution::distribute(&lookup, &system, &closure, &mut reporter);
            closure
        });
    system.placements = placement::place(&lookup, &system, system.closure.as_ref(), &mut reporter);
    check_depths(db, ws, std, &system, &mut reporter);
    system.component_lines = lines;
    // rsdl §13: an RSDL-7xx error blocks its own deployment, recorded on its
    // placement; every other error blocks every deployment.
    system.closure_has_errors = reporter.diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == Severity::Error && !diagnostic.code.as_str().starts_with("RSDL-7")
    });
    system.diagnostics = reporter.diagnostics;
    system
}

/// RSDL-805 and RSDL-806 (rsdl §5): the `depth` of every consumer link of
/// an event, against the event's contract bound, `ceil(max / min)` over its
/// resolved timing (`docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md`
/// decision 21).
///
/// The link's declared `depth` is the placement line's value, else the
/// deployment's: the precedence `ridl_ir::codegen::lower_deployment` applies
/// when it writes the deployment section (its `declared_sizing`). The two
/// read the same two sites in the same order and are edited together. A
/// declared value below the bound is RSDL-805, pointed at the value that
/// declares it: the line's `depth` attribute, or the deployment's name. An
/// event with no derivable bound — an explicit half-open range, or a ratio
/// `ceil_ratio` refuses — consumed by a link with no declared value is
/// RSDL-806, pointed at the placement line.
///
/// Each code is drawn once per consumer instance and event: the value and
/// the bound belong to the consumer and the event, so a redundant provider
/// set (RSDL-409), which lowers one link per producer instance, does not
/// repeat them. A deployment an RSDL-7xx error blocks is skipped, because
/// its placement set is not one to read links from: an instance placed
/// twice or not at all, a machine declared twice, or a sizing value out of
/// range leaves the lines of that deployment without a defined link set
/// (rsdl §13). A `requires` whose two ends are external is skipped too: it
/// lowers no link (rsdl §10).
fn check_depths(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    system: &CheckedSystem,
    reporter: &mut Reporter,
) {
    let Some(closure) = &system.closure else {
        return;
    };
    // The events of each required interface, read once per `requires` line.
    let events: Vec<Vec<EventBound>> = closure
        .requires
        .iter()
        .map(|require| {
            let consumer = &closure.components[require.consumer];
            let producer = &closure.components[require.producer];
            if consumer.external && producer.external {
                return Vec::new();
            }
            event_bounds(db, ws, std, closure, require)
        })
        .collect();
    for (decl, placement) in system.deployments.iter().zip(&system.placements) {
        if placement.has_errors {
            continue;
        }
        for (require, events) in closure.requires.iter().zip(&events) {
            let consumer = &closure.components[require.consumer];
            for instance in &consumer.instances {
                let Some(placed) = placement.placements.iter().find(|placed| {
                    placed.component == require.consumer && placed.instance == *instance
                }) else {
                    continue;
                };
                let line = &decl.machines[placed.machine].members[placed.line];
                let declared = line
                    .sizing
                    .depth
                    .map(|value| (value, line.depth_site.unwrap_or(line.reference.site)))
                    .or_else(|| decl.sizing.depth.map(|value| (value, decl.name.site)));
                let name = if *instance == UNIT_INSTANCE {
                    consumer.id.text().to_string()
                } else {
                    format!("{}.{instance}", consumer.id.text())
                };
                for event in events {
                    match (declared, event.bound) {
                        (Some((value, site)), Ok(bound)) if value < bound => reporter.warning(
                            DiagCode::RSDL_805,
                            site,
                            format!(
                                "`depth = {value}` on `{name}` is below the contract bound {bound} \
                                 of `{}` — the bound is `ceil(max / min)` over the event's timing, \
                                 and a ring below it can drop occurrences alive at once (rsdl \
                                 reference §5)",
                                event.name
                            ),
                        ),
                        (None, Err(reason)) => reporter.warning(
                            DiagCode::RSDL_806,
                            line.reference.site,
                            format!(
                                "`{name}` declares no `depth` for `{}`, {reason} — a link to an \
                                 event with no derivable bound takes its ring depth from \
                                 `depth = <n>` on the placement line or on the deployment (rsdl \
                                 reference §5)",
                                event.name
                            ),
                        ),
                        _ => {}
                    }
                }
            }
        }
    }
}

/// One event of a required interface, with its contract bound or the reason
/// it has none, as the RSDL-806 message states it.
struct EventBound {
    /// `pkg.Interface.event`, or `service.event` for an inline shape.
    name: String,
    bound: Result<u32, &'static str>,
}

/// The events of the interface `require` names, read from the package IR of
/// the package that declares it, in declaration order. Empty when the
/// package or the shape is not found, or when the interface declares no
/// event.
fn event_bounds(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    closure: &Closure,
    require: &ResolvedRequire,
) -> Vec<EventBound> {
    let (package, shape_name) = match &require.interface {
        InterfaceId::Declared { package, name } => (package.as_str(), name.as_str()),
        InterfaceId::Inline { service } => {
            (closure.services[service].package.as_str(), service.as_str())
        }
    };
    let Some(pkg) = ws
        .packages(db)
        .iter()
        .chain(std::iter::once(&std))
        .copied()
        .find(|pkg| pkg.name(db) == package)
    else {
        return Vec::new();
    };
    let ir = crate::check_package(db, ws, pkg, std).ir;
    let interface = require.interface.text();
    let Some(shape) = ir.shapes().find(|found| found.name == shape_name) else {
        return Vec::new();
    };
    shape
        .interface
        .interactions
        .iter()
        .filter_map(|decl| match decl.kind.as_ref()? {
            v2::decl::Kind::EventDef(event) => Some(EventBound {
                name: format!("{interface}.{}", decl.name),
                bound: contract_bound(event.timing.as_ref()),
            }),
            _ => None,
        })
        .collect()
}

/// The contract bound of an event, `ceil(max / min)` over its resolved
/// timing, or the reason it has none. The resolved timing of an untimed
/// event carries the ridl §9.1 defaults, so only an explicit half-open range
/// lacks a bound. A ratio `ceil_ratio` refuses — above the `depth` range, or
/// any other refusal it may add — is reported without naming a cause, because
/// `ceil_ratio` does not say which one it found.
fn contract_bound(timing: Option<&v2::Timing>) -> Result<u32, &'static str> {
    let Some((max, min)) =
        timing.and_then(|timing| Some((timing.max_us.as_deref()?, timing.min_us.as_deref()?)))
    else {
        return Err("whose timing is an explicit half-open range");
    };
    ceil_ratio(max, min)
        .ok_or("whose contract bound `ceil(max / min)` is not derivable from its timing")
}

/// Runs the doc lints (ADR-0026) over every `.rsdl` file of `ws`, in
/// package-then-file order. `check_package` lints the other files.
fn lint_docs(db: &dyn salsa::Database, ws: Workspace, reporter: &mut Reporter) {
    for package in ws.packages(db) {
        for file in package.files(db) {
            if profile_of_path(file.path(db)) != Profile::Rsdl {
                continue;
            }
            let file_id = reporter
                .file_ids
                .get(file)
                .copied()
                .unwrap_or(FileId::DETACHED);
            let source = source_file(db, *file);
            reporter
                .diagnostics
                .extend(crate::doc_lint::lint_rsdl_file(&source, file_id));
        }
    }
}

/// The checked rsdl model of one workspace. Each list holds its declarations
/// in package-then-file order, then source order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedSystem {
    /// Every `system` declaration (rsdl §3.1).
    pub systems: Vec<SystemDecl>,
    /// Every `component` declaration (rsdl §3.2).
    pub components: Vec<ComponentDecl>,
    /// Every `distribution` declaration (rsdl §3.3).
    pub distributions: Vec<DistributionDecl>,
    /// Every `deployment` declaration, with its machines (rsdl §3.4, §3.5).
    pub deployments: Vec<DeploymentDecl>,
    /// The resolved lines of every component, parallel to `components`
    /// (rsdl §3.2).
    pub component_lines: Vec<ComponentLines>,
    /// The closure of the first `system`, or `None` when the workspace declares
    /// none (rsdl §3.1).
    pub closure: Option<Closure>,
    /// The placement of the closure in each deployment, parallel to
    /// `deployments` (rsdl §9).
    pub placements: Vec<DeploymentPlacement>,
    /// Whether an error other than an RSDL-7xx one was raised: rsdl §13 blocks
    /// lowering for every deployment then. An error in an attribute block
    /// (a FORM code) counts here, wherever the block is written.
    pub closure_has_errors: bool,
    /// The rsdl diagnostics, with the workspace file ids described in the
    /// module documentation.
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckedSystem {
    /// Every backend key written on a declaration or a line, in the order of
    /// the declaration lists (rsdl §5). `ridlc` reads this for RSDL-804.
    pub fn backend_keys(&self) -> Vec<&BackendKey> {
        let mut keys = Vec::new();
        for system in &self.systems {
            keys.extend(&system.attrs.backend_keys);
            for line in &system.members {
                keys.extend(&line.backend_keys);
            }
        }
        for component in &self.components {
            keys.extend(&component.attrs.backend_keys);
            for line in component.offers.iter().chain(&component.requires) {
                keys.extend(&line.backend_keys);
            }
        }
        for distribution in &self.distributions {
            keys.extend(&distribution.attrs.backend_keys);
            for line in &distribution.members {
                keys.extend(&line.backend_keys);
            }
        }
        for deployment in &self.deployments {
            keys.extend(&deployment.attrs.backend_keys);
            for machine in &deployment.machines {
                keys.extend(&machine.attrs.backend_keys);
                for line in &machine.members {
                    keys.extend(&line.backend_keys);
                }
            }
        }
        keys
    }
}

/// RSDL-804 (rsdl reference §5): one warning for every backend key of `system`
/// whose namespace is not in `claimed`, the namespaces the configured backends
/// consume. The key stays in the model either way, and the warning never
/// blocks (rsdl §13). The spans are interned into `sources`, so the returned
/// diagnostics render against it with no remap.
///
/// This is not part of [`check_system`]: only a driver knows which backends are
/// configured, so `ridlc` and the language server call it with the set they
/// claim.
pub fn unclaimed_backend_keys(
    db: &dyn salsa::Database,
    system: &CheckedSystem,
    claimed: &BTreeSet<String>,
    sources: &mut SourceMap,
) -> Vec<Diagnostic> {
    system
        .backend_keys()
        .into_iter()
        .filter(|key| !claimed.contains(&key.namespace))
        .map(|key| {
            let file = key.site.file;
            Diagnostic {
                code: DiagCode::RSDL_804,
                severity: Severity::Warning,
                message: format!(
                    "no configured backend claims the namespace `{}`, so `{}.{}` is carried \
                     uninterpreted (rsdl reference §5)",
                    key.namespace, key.namespace, key.key
                ),
                primary: Span {
                    file: sources.file_id(file.path(db), file.text(db)),
                    range: key.site.range,
                },
                labels: Vec::new(),
                fixits: Vec::new(),
            }
        })
        .collect()
}

/// Where a model entry is written: its file and its byte range in that file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Site {
    pub file: InputFile,
    pub range: TextRange,
}

/// A name as declared, with its site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    pub name: String,
    pub site: Site,
}

/// A `system` declaration: the closure (rsdl §3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemDecl {
    pub name: Named,
    /// The package of the file that declares it.
    pub package: String,
    /// The member lines, in source order.
    pub members: Vec<MemberRef>,
    pub attrs: DeclAttrs,
    /// The doc comment (typl §14, ADR-0026).
    pub doc: DocInfo,
    /// The resolved doc links of the body, for the IR (ADR-0026).
    pub links: Vec<v2::DocLink>,
    /// The resolved `@see` targets, for the IR (ADR-0026).
    pub see: Vec<v2::DocLink>,
}

/// A `component` declaration (rsdl §3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDecl {
    pub name: Named,
    pub package: String,
    /// The `offers` lines, in source order.
    pub offers: Vec<MemberRef>,
    /// The `requires` lines, in source order.
    pub requires: Vec<MemberRef>,
    /// `None` when the component declares no `instances`. `Some` whenever the
    /// key is written, holding the items that are camelCase names or `Unit`,
    /// in source order; every other item drew RSDL-305, so an empty list always
    /// comes with an RSDL-305.
    pub instances: Option<Vec<Named>>,
    /// The `external` flag (rsdl §5). A written `external = …` drew RSDL-313
    /// and still sets the flag.
    pub external: bool,
    pub attrs: DeclAttrs,
    /// The doc comment (typl §14, ADR-0026).
    pub doc: DocInfo,
    /// The resolved doc links of the body, for the IR (ADR-0026).
    pub links: Vec<v2::DocLink>,
    /// The resolved `@see` targets, for the IR (ADR-0026).
    pub see: Vec<v2::DocLink>,
}

/// A `distribution` declaration (rsdl §3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributionDecl {
    pub name: Named,
    pub package: String,
    pub members: Vec<MemberRef>,
    /// `None` when `tier` is absent, or when its value drew RSDL-908.
    pub tier: Option<Tier>,
    pub attrs: DeclAttrs,
    /// The doc comment (typl §14, ADR-0026).
    pub doc: DocInfo,
    /// The resolved doc links of the body, for the IR (ADR-0026).
    pub links: Vec<v2::DocLink>,
    /// The resolved `@see` targets, for the IR (ADR-0026).
    pub see: Vec<v2::DocLink>,
}

/// A `deployment` declaration and its machines (rsdl §3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentDecl {
    pub name: Named,
    pub package: String,
    /// The reference after `for`; `None` when the parser reported it missing.
    pub system: Option<Reference>,
    pub machines: Vec<MachineDecl>,
    pub attrs: DeclAttrs,
    /// The `depth`, `slots` and `budget` keys of the declaration: the values
    /// for every consumer link of the deployment that no placement line
    /// overrides (rsdl §5).
    pub sizing: Sizing,
    /// Whether an RSDL-7xx error was raised while reading the declaration's
    /// attribute blocks — RSDL-709 on the deployment or on one of its
    /// placement lines. The placement pass marks the deployment blocked
    /// (rsdl §13).
    pub has_errors: bool,
    /// The doc comment (typl §14, ADR-0026).
    pub doc: DocInfo,
    /// The resolved doc links of the body, for the IR (ADR-0026).
    pub links: Vec<v2::DocLink>,
    /// The resolved `@see` targets, for the IR (ADR-0026).
    pub see: Vec<v2::DocLink>,
}

/// The sizing keys of a `deployment` declaration or of a placement line
/// (rsdl §5): each is `None` when the key is not written. `depth` sizes the
/// ring of an event channel, `slots` and `budget` the call table of a command
/// or query channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Sizing {
    /// `depth = n`, 1 to 4294967295.
    pub depth: Option<u32>,
    /// `slots = n`, 1 to 65536.
    pub slots: Option<u32>,
    /// `budget = n`, 1 to 18446744073709551615.
    pub budget: Option<u64>,
}

/// A `machine` declaration inside a deployment (rsdl §3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineDecl {
    pub name: Named,
    /// The placement lines, in source order.
    pub members: Vec<MemberRef>,
    /// The `external` flag, read as on a component.
    pub external: bool,
    pub attrs: DeclAttrs,
    /// The doc comment (typl §14, ADR-0026).
    pub doc: DocInfo,
    /// The resolved doc links of the body, for the IR (ADR-0026).
    pub links: Vec<v2::DocLink>,
    /// The resolved `@see` targets, for the IR (ADR-0026).
    pub see: Vec<v2::DocLink>,
}

/// One body line: a member line of a `system`, `distribution` or `machine`,
/// or an `offers`/`requires` line of a `component` (rsdl §4). A line takes
/// backend keys; a placement line, a member line of a `machine`, also takes
/// the sizing keys (rsdl §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRef {
    pub reference: Reference,
    pub backend_keys: Vec<BackendKey>,
    /// The `depth`, `slots` and `budget` keys of a placement line, for every
    /// link the placed instance consumes (rsdl §5). Never written on any
    /// other line.
    pub sizing: Sizing,
    /// The site of the line's `depth` attribute, `Some` exactly when
    /// `sizing.depth` is; RSDL-805 points at it.
    pub depth_site: Option<Site>,
    /// The line's doc comment (typl §14, ADR-0026).
    pub doc: DocInfo,
    /// The resolved doc links of the line, for the IR (ADR-0026).
    pub links: Vec<v2::DocLink>,
    /// The resolved `@see` targets, for the IR (ADR-0026).
    pub see: Vec<v2::DocLink>,
}

/// A reference as written (rsdl §4): its dotted segments, its site, and the
/// role its case reads. Lookup confirms the role in the pass that resolves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub segments: Vec<String>,
    pub site: Site,
    pub form: ReferenceForm,
}

impl Reference {
    /// The reference as written, segments joined by `.`.
    pub fn text(&self) -> String {
        self.segments.join(".")
    }
}

/// The role a reference's case reads (rsdl §4 table, general form R7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceForm {
    /// `Name` or `pkg.Name`: a component — after `for`, the system; after
    /// `requires`, an interface. `package` holds the lowercase leading
    /// segments joined by `.`, `None` for a bare name.
    Declared {
        package: Option<String>,
        name: String,
    },
    /// `Name.inst` or `pkg.Name.inst`: one instance of a component. The
    /// instance segment is camelCase, or the written `Unit` that RSDL-307
    /// reports.
    Instance {
        package: Option<String>,
        component: String,
        instance: String,
    },
    /// `pkg.service`: lowercase segments only, a service's global dotted name.
    Service { name: String },
    /// A shape no row of the rsdl §4 table names. The slot's unknown-name code
    /// reports it.
    Unreadable,
}

/// A distribution's `tier` (rsdl §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    Platform,
    Application,
}

/// The attributes of a declaration that the rsdl model carries beside its own
/// facts (rsdl §5): the family's `labels` and `deprecated`, and every backend
/// key as written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeclAttrs {
    /// `labels = (…)`: the labels in source order.
    pub labels: Vec<String>,
    /// `deprecated = "…"`: the text between the quotes, escapes as written.
    pub deprecated: Option<String>,
    /// Every `namespace.key` attribute, in source order.
    pub backend_keys: Vec<BackendKey>,
}

/// A backend key (rsdl §5): carried as declared, never interpreted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendKey {
    /// `linux` in `linux.cpuset`.
    pub namespace: String,
    /// `cpuset` in `linux.cpuset`.
    pub key: String,
    /// The value, or `None` for a flag.
    pub value: Option<WrittenValue>,
    /// The whole attribute.
    pub site: Site,
}

/// An attribute value as written (rsdl Appendix B `attr_value`): the text of
/// one literal or name, or a parenthesised list of values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WrittenValue {
    Scalar(String),
    List(Vec<WrittenValue>),
}

/// Stamps rsdl diagnostics with the workspace file ids.
struct Reporter {
    file_ids: HashMap<InputFile, FileId>,
    diagnostics: Vec<Diagnostic>,
}

impl Reporter {
    /// Interns every file of `ws` in package-then-file order.
    fn new(db: &dyn salsa::Database, ws: Workspace) -> Self {
        let mut sources = SourceMap::new();
        let mut file_ids = HashMap::new();
        for package in ws.packages(db) {
            for file in package.files(db) {
                file_ids.insert(*file, sources.file_id(file.path(db), file.text(db)));
            }
        }
        Self {
            file_ids,
            diagnostics: Vec::new(),
        }
    }

    fn error(&mut self, code: DiagCode, site: Site, message: String) {
        self.push(code, Severity::Error, site, message);
    }

    fn warning(&mut self, code: DiagCode, site: Site, message: String) {
        self.push(code, Severity::Warning, site, message);
    }

    fn push(&mut self, code: DiagCode, severity: Severity, site: Site, message: String) {
        let file = self
            .file_ids
            .get(&site.file)
            .copied()
            .unwrap_or(FileId::DETACHED);
        self.diagnostics.push(Diagnostic {
            code,
            severity,
            message,
            primary: Span {
                file,
                range: site.range,
            },
            labels: Vec::new(),
            fixits: Vec::new(),
        });
    }
}

/// `CamelCase_id` (typl Appendix E): an upper-case letter, then letters and
/// digits.
fn is_upper_camel(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase()) && chars.all(|c| c.is_ascii_alphanumeric())
}

/// `camelCase_id` (typl Appendix E): a lower-case letter, then letters and
/// digits. No underscore.
fn is_lower_camel(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase()) && chars.all(|c| c.is_ascii_alphanumeric())
}

/// A package or service name segment (ADR-0002 §1): a lower-case letter, then
/// lower-case letters and digits.
fn is_lowercase_segment(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// `SCREAMING_SNAKE_ID` (typl Appendix E): an upper-case letter, then
/// upper-case letters, digits and underscores.
fn is_screaming_snake(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ridl_core::db::RidlDatabase;
    use ridl_core::package::PackageOrigin;
    use ridl_core::std_package;

    use super::*;

    /// A package named `name` holding `files` as `(path, text)` pairs.
    pub(super) fn package(db: &RidlDatabase, name: &str, files: &[(&str, &str)]) -> Package {
        let files = files
            .iter()
            .map(|(path, text)| InputFile::new(db, path.to_string(), text.to_string()))
            .collect();
        Package::new(
            db,
            name.to_string(),
            files,
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            None,
            None,
        )
    }

    /// Checks a workspace made of `packages`, each `(name, files)`. TYPL-406
    /// (`missing-docs`) is left out: the fixtures have no docs, and these
    /// tests are about the rsdl checks.
    fn check(packages: &[(&str, &[(&str, &str)])]) -> CheckedSystem {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let packages = packages
            .iter()
            .map(|(name, files)| package(&db, name, files))
            .collect();
        let ws = Workspace::new(&db, packages, BTreeMap::new());
        let mut system = check_system(&db, ws, std);
        system
            .diagnostics
            .retain(|diagnostic| diagnostic.code != DiagCode::TYPL_406);
        system
    }

    fn codes(system: &CheckedSystem) -> Vec<&str> {
        system.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    /// The codes of `system`'s errors, warnings left out.
    fn errors(system: &CheckedSystem) -> Vec<&str> {
        system
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.code.as_str())
            .collect()
    }

    /// rsdl reference Appendix A, `system.rsdl`, verbatim.
    pub(super) const SYSTEM: &str = r#"package veh.topology

import veh.adas.CruiseControl
import veh.adas.LaneAssist

/// Adaptive cruise, two copies: one per compute node.
component Cruise [ instances = (primary, backup), labels = (ASIL_B) ] {
  offers   veh.adas.cruise
  requires LaneAssist
}

component Lane { offers veh.adas.lane }

component Panel {
  requires CruiseControl
  requires LaneAssist
}

/// The fleet backend. No implementation in this workspace.
component Backend [ external ] {
  requires CruiseControl
  requires veh.diag.access
}

system Vehicle { Cruise, Lane, Panel, Backend, veh.diag.access }

distribution Adas [ tier = PLATFORM ]    { Cruise, Lane, veh.diag.access }
distribution Hmi  [ tier = APPLICATION ] { Panel }
"#;

    /// rsdl reference Appendix A, `production.rsdl`, with the same package.
    pub(super) const PRODUCTION: &str = r#"package veh.topology

deployment Production for Vehicle {
  machine AdasHpc [ labels = (ASIL_B) ] { Cruise.primary, Lane, veh.diag.access }
  machine Cockpit { Cruise.backup, Panel [ linux.cpuset = (2, 3) ] }
  machine Cloud   [ external ] { Backend }
}
"#;

    fn texts(lines: &[MemberRef]) -> Vec<String> {
        lines.iter().map(|line| line.reference.text()).collect()
    }

    /// The doc lints run over each `.rsdl` file once: `check_system` reports
    /// the `/** */` doc of the `.rsdl` file against that file's id, and leaves
    /// the `.typl` file to `check_package`.
    #[test]
    fn a_block_doc_in_an_rsdl_file_draws_typl_410_once() {
        let typl = "package veh\n\n/** A speed. */\ntype Speed: integer [0..300]\n";
        let rsdl = "package veh\n\n/** The panel. */\ncomponent Panel {}\n";
        let system = check(&[("veh", &[("veh/a.typl", typl), ("veh/b.rsdl", rsdl)])]);
        let found: Vec<&Diagnostic> = system
            .diagnostics
            .iter()
            .filter(|d| d.code == DiagCode::TYPL_410)
            .collect();
        assert_eq!(found.len(), 1, "{:?}", system.diagnostics);
        let mut sources = SourceMap::new();
        sources.file_id("veh/a.typl", typl);
        let rsdl_id = sources.file_id("veh/b.rsdl", rsdl);
        assert_eq!(found[0].primary.file, rsdl_id);
        assert_eq!(found[0].fixits[0].replacement, "/// The panel.");
    }

    #[test]
    fn appendix_a_collects_into_the_model_with_no_error() {
        let system = check_topology(&[
            ("veh/topology/system.rsdl", SYSTEM),
            ("veh/topology/production.rsdl", PRODUCTION),
        ]);
        assert_eq!(errors(&system), Vec::<&str>::new());

        let [vehicle] = system.systems.as_slice() else {
            panic!("one system, got {:?}", system.systems);
        };
        assert_eq!(vehicle.name.name, "Vehicle");
        assert_eq!(vehicle.package, "veh.topology");
        assert_eq!(
            texts(&vehicle.members),
            ["Cruise", "Lane", "Panel", "Backend", "veh.diag.access"]
        );

        let names: Vec<&str> = system
            .components
            .iter()
            .map(|c| c.name.name.as_str())
            .collect();
        assert_eq!(names, ["Cruise", "Lane", "Panel", "Backend"]);
        let cruise = &system.components[0];
        assert_eq!(texts(&cruise.offers), ["veh.adas.cruise"]);
        assert_eq!(texts(&cruise.requires), ["LaneAssist"]);
        let instances: Vec<&str> = cruise
            .instances
            .as_ref()
            .expect("Cruise declares instances")
            .iter()
            .map(|named| named.name.as_str())
            .collect();
        assert_eq!(instances, ["primary", "backup"]);
        assert_eq!(cruise.attrs.labels, ["ASIL_B"]);
        assert!(!cruise.external);
        assert_eq!(system.components[1].instances, None);
        let backend = &system.components[3];
        assert!(backend.external);
        assert_eq!(
            texts(&backend.requires),
            ["CruiseControl", "veh.diag.access"]
        );
        assert_eq!(
            backend.requires[1].reference.form,
            ReferenceForm::Service {
                name: "veh.diag.access".to_string()
            }
        );

        let tiers: Vec<_> = system.distributions.iter().map(|d| d.tier).collect();
        assert_eq!(tiers, [Some(Tier::Platform), Some(Tier::Application)]);

        let [production] = system.deployments.as_slice() else {
            panic!("one deployment, got {:?}", system.deployments);
        };
        assert_eq!(
            production.system.as_ref().map(Reference::text).as_deref(),
            Some("Vehicle")
        );
        let machines: Vec<(&str, bool)> = production
            .machines
            .iter()
            .map(|m| (m.name.name.as_str(), m.external))
            .collect();
        assert_eq!(
            machines,
            [("AdasHpc", false), ("Cockpit", false), ("Cloud", true)]
        );
        assert_eq!(production.machines[0].attrs.labels, ["ASIL_B"]);
        assert_eq!(
            production.machines[0].members[0].reference.form,
            ReferenceForm::Instance {
                package: None,
                component: "Cruise".to_string(),
                instance: "primary".to_string(),
            }
        );

        // The one backend key: on `Panel`'s placement line in `Cockpit`.
        let keys = system.backend_keys();
        let [key] = keys.as_slice() else {
            panic!("one backend key, got {keys:?}");
        };
        assert_eq!(
            (key.namespace.as_str(), key.key.as_str()),
            ("linux", "cpuset")
        );
        assert_eq!(
            key.value,
            Some(WrittenValue::List(vec![
                WrittenValue::Scalar("2".to_string()),
                WrittenValue::Scalar("3".to_string()),
            ]))
        );
        assert_eq!(
            production.machines[1].members[1].backend_keys,
            [(*key).clone()]
        );
    }

    /// Every attribute rule of rsdl §5, one input per rule, with the codes it
    /// draws.
    #[test]
    fn attribute_keys_follow_the_rsdl_allow_list() {
        let cases: &[(&str, &[&str])] = &[
            // Legal on each kind: no diagnostic.
            (
                "component C [ instances = (a, b), external, labels = (QM), deprecated = \"x\" ] {}",
                &[],
            ),
            ("machine M [ external, labels = (QM) ] {}", &[]),
            (
                "system S [ labels = (QM), deprecated = \"x\", rust.crate = \"s\" ] { C [ linux.cpuset = (2) ] }",
                &[],
            ),
            (
                "distribution D [ tier = APPLICATION ] { C [ sign.key ] }",
                &[],
            ),
            (
                "component C { offers veh.a.b [ someip.serviceId = 4660 ] }",
                &[],
            ),
            // FORM-106: a key no row and no backend namespace defines.
            ("system S [ owner = \"x\" ] {}", &["FORM-106"]),
            ("system S [ Linux.cpuset ] {}", &["FORM-106"]),
            ("system S [ linux.cpu_set ] {}", &["FORM-106"]),
            // FORM-107: a key its row does not name, and any rsdl key on a line
            // (the sizing keys of a placement line aside).
            ("component C [ tier = PLATFORM ] {}", &["FORM-107"]),
            ("system S [ external ] {}", &["FORM-107"]),
            ("distribution D [ instances = (a) ] {}", &["FORM-107"]),
            ("system S { C [ deprecated = \"x\" ] }", &["FORM-107"]),
            (
                "component C { requires I [ labels = (QM) ] }",
                &["FORM-107"],
            ),
            // FORM-108: a key twice in one block, backend keys included.
            (
                "system S [ labels = (QM), labels = (QM) ] {}",
                &["FORM-108"],
            ),
            ("system S [ rust.crate, rust.crate ] {}", &["FORM-108"]),
            // RSDL-305: `instances` is a parenthesised list of camelCase names.
            ("component C [ instances = () ] {}", &["RSDL-305"]),
            ("component C [ instances = solo ] {}", &["RSDL-305"]),
            ("component C [ instances ] {}", &["RSDL-305"]),
            (
                "component C [ instances = (primary, Backup, \"x\") ] {}",
                &["RSDL-305", "RSDL-305"],
            ),
            // A written `Unit` is kept for RSDL-307, not reported here.
            ("component C [ instances = (Unit) ] {}", &[]),
            // RSDL-313: `external` is a flag.
            ("component C [ external = true ] {}", &["RSDL-313"]),
            ("machine M [ external = false ] {}", &["RSDL-313"]),
            // RSDL-908: `tier` is `PLATFORM` or `APPLICATION`.
            ("distribution D [ tier = SYSTEM ] {}", &["RSDL-908"]),
            ("distribution D [ tier ] {}", &["RSDL-908"]),
            // FORM-101: `labels` and `deprecated` of another shape.
            ("system S [ labels = QM ] {}", &["FORM-101"]),
            ("system S [ deprecated = 3 ] {}", &["FORM-101"]),
        ];
        for (decl, expected) in cases {
            let text = if decl.starts_with("machine") {
                format!("package p\ndeployment X for S {{\n  {decl}\n}}\n")
            } else {
                format!("package p\n{decl}\n")
            };
            let system = check(&[("p", &[("p/x.rsdl", text.as_str())])]);
            // The closure, placement and distribution checks of the later
            // passes also read these inputs; this test pins the attribute
            // rules alone.
            let attribute_codes: Vec<&str> = codes(&system)
                .into_iter()
                .filter(|code| {
                    code.starts_with("FORM-") || ["RSDL-305", "RSDL-313", "RSDL-908"].contains(code)
                })
                .collect();
            assert_eq!(attribute_codes, *expected, "`{decl}`");
        }
    }

    /// The flag survives RSDL-313 and the valid instance names survive RSDL-305,
    /// so a later pass reads what the author meant.
    #[test]
    fn a_rejected_value_keeps_what_can_be_read() {
        let system = check(&[(
            "p",
            &[(
                "p/x.rsdl",
                "package p\ncomponent C [ external = true, instances = (primary, Backup) ] {}\n",
            )],
        )]);
        assert_eq!(codes(&system), ["RSDL-313", "RSDL-305"]);
        let component = &system.components[0];
        assert!(component.external);
        let names: Vec<&str> = component
            .instances
            .as_ref()
            .expect("the key is written")
            .iter()
            .map(|named| named.name.as_str())
            .collect();
        assert_eq!(names, ["primary"]);
    }

    /// A diagnostic's file id indexes the workspace's files in
    /// package-then-file order, the order `service_catalog` uses, so a driver
    /// remaps both with the same render ids.
    #[test]
    fn diagnostics_index_the_workspace_files_in_package_then_file_order() {
        let first: &[(&str, &str)] = &[
            ("a/one.typl", "package a\ntype A: m\n"),
            ("a/two.ridl", "package a\ninterface I {}\n"),
        ];
        let second: &[(&str, &str)] = &[("b/x.rsdl", "package b\nsystem S [ owner ] {}\n")];
        let system = check(&[("a", first), ("b", second)]);
        let [diagnostic] = system.diagnostics.as_slice() else {
            panic!("one diagnostic, got {:?}", system.diagnostics);
        };
        let mut sources = SourceMap::new();
        for (path, text) in first.iter().chain(second) {
            sources.file_id(path, text);
        }
        assert_eq!(sources.path(diagnostic.primary.file), Some("b/x.rsdl"));
    }

    /// rsdl reference Appendix A, `veh/common/common.typl`.
    pub(super) const COMMON: &str = r#"package veh.common

type Speed: km/h [0.0..250.0 step 0.5]
type Engaged: boolean
enum LeverCmd { SET = 1, CANCEL = 2, RESUME = 3 }
struct FaultReport { count: integer [0..255] }
"#;

    /// rsdl reference Appendix A, `veh/adas/adas.ridl`.
    pub(super) const ADAS: &str = r#"package veh.adas

import veh.common.Engaged
import veh.common.Speed
import veh.common.LeverCmd

interface CruiseControl {
  signal  engaged: Engaged @[100ms..1s]
  signal  target: Speed @[100ms..1s]
  command setLever(cmd: LeverCmd) @[..50ms]
}
interface LaneAssist {
  signal active: Engaged @[100ms..1s]
}

service veh.adas.cruise : CruiseControl
service veh.adas.lane   : LaneAssist
"#;

    /// rsdl reference Appendix A, `veh/diag/diag.ridl`: one inline-shape
    /// service.
    pub(super) const DIAG: &str = r#"package veh.diag

import veh.common.FaultReport

service veh.diag.access {
  query readFaults(): FaultReport @[..100ms]
}
"#;

    /// Checks Appendix A's three contract packages beside `topology`, the files
    /// of the package `veh.topology`.
    fn check_topology(topology: &[(&str, &str)]) -> CheckedSystem {
        let common: &[(&str, &str)] = &[("veh/common/common.typl", COMMON)];
        let adas: &[(&str, &str)] = &[("veh/adas/adas.ridl", ADAS)];
        let diag: &[(&str, &str)] = &[("veh/diag/diag.ridl", DIAG)];
        check(&[
            ("veh.common", common),
            ("veh.adas", adas),
            ("veh.diag", diag),
            ("veh.topology", topology),
        ])
    }

    fn declared_interface(package: &str, name: &str) -> InterfaceId {
        InterfaceId::Declared {
            package: package.to_string(),
            name: name.to_string(),
        }
    }

    /// Appendix A's closure: four declared components and the implicit
    /// component of `veh.diag.access`, its services with their offering
    /// components, and its interfaces with their owning services (rsdl §3.1,
    /// §6, §7, and the "Closure" and "Instances" items after the example).
    #[test]
    fn appendix_a_derives_its_closure() {
        let system = check_topology(&[("veh/topology/system.rsdl", SYSTEM)]);
        assert_eq!(errors(&system), Vec::<&str>::new());
        let closure = system
            .closure
            .as_ref()
            .expect("Appendix A declares a system");

        let components: Vec<(&str, Vec<&str>, bool)> = closure
            .components
            .iter()
            .map(|component| {
                let instances = component.instances.iter().map(String::as_str).collect();
                (component.id.text(), instances, component.external)
            })
            .collect();
        assert_eq!(
            components,
            [
                ("Cruise", vec!["primary", "backup"], false),
                ("Lane", vec!["Unit"], false),
                ("Panel", vec!["Unit"], false),
                ("Backend", vec!["Unit"], true),
                ("veh.diag.access", vec!["Unit"], false),
            ]
        );
        assert_eq!(closure.components[0].decl, Some(0));
        assert_eq!(closure.components[4].decl, None);

        let services: Vec<(&str, &[usize])> = closure
            .services
            .iter()
            .map(|(name, service)| (name.as_str(), service.offerers.as_slice()))
            .collect();
        assert_eq!(
            services,
            [
                ("veh.adas.cruise", &[0][..]),
                ("veh.adas.lane", &[1][..]),
                ("veh.diag.access", &[4][..]),
            ]
        );
        let interfaces: Vec<(String, &[String])> = closure
            .interface_owners
            .iter()
            .map(|(interface, owners)| (interface.text(), owners.as_slice()))
            .collect();
        assert_eq!(
            interfaces,
            [
                (
                    "veh.adas.CruiseControl".to_string(),
                    &["veh.adas.cruise".to_string()][..]
                ),
                (
                    "veh.adas.LaneAssist".to_string(),
                    &["veh.adas.lane".to_string()][..]
                ),
                (
                    "veh.diag.access".to_string(),
                    &["veh.diag.access".to_string()][..]
                ),
            ]
        );

        // A bare interface name resolves through the file's imports; a service
        // with an inline shape names its one interface (rsdl §3.2).
        assert_eq!(
            system.component_lines[2].requires,
            [
                Some(declared_interface("veh.adas", "CruiseControl")),
                Some(declared_interface("veh.adas", "LaneAssist")),
            ]
        );
        assert_eq!(
            system.component_lines[3].requires[1],
            Some(InterfaceId::Inline {
                service: "veh.diag.access".to_string()
            })
        );
    }

    /// The `system` rules (rsdl §3.1, §6, §7), one input per rule.
    #[test]
    fn a_system_lists_each_component_or_lone_service_once() {
        const COMPONENTS: &str = "component Cruise [ instances = (primary, backup) ] { offers veh.adas.cruise }\n\
                                  component Lane { offers veh.adas.lane }\n";
        let cases: &[(&str, &[&str])] = &[
            ("system Vehicle { Cruise, Lane, veh.diag.access }", &[]),
            (
                "system Vehicle { Cruise }\nsystem Bench { Lane }",
                &["RSDL-601"],
            ),
            ("system Vehicle { Brake }", &["RSDL-602"]),
            ("system Vehicle { veh.adas.brake }", &["RSDL-602"]),
            ("system Vehicle { Cruise.primary }", &["RSDL-602"]),
            ("system Vehicle { Lane, Lane }", &["RSDL-603"]),
            ("system Vehicle { Lane, veh.topology.Lane }", &["RSDL-603"]),
            (
                "system Vehicle { veh.diag.access, veh.diag.access }",
                &["RSDL-603"],
            ),
            ("system Vehicle { Lane, veh.adas.cruise }", &["RSDL-504"]),
            ("system Vehicle { Lane.Unit }", &["RSDL-307"]),
        ];
        for (decl, expected) in cases {
            let text = format!("package veh.topology\n{COMPONENTS}{decl}\n");
            let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
            assert_eq!(codes(&system), *expected, "`{decl}`");
        }

        // RSDL-504 names the offerer.
        let text =
            format!("package veh.topology\n{COMPONENTS}system Vehicle {{ veh.adas.cruise }}\n");
        let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
        assert!(
            system.diagnostics[0]
                .message
                .contains("offered by the component `Cruise`"),
            "got: {}",
            system.diagnostics[0].message
        );
    }

    /// The line and instance rules of a component (rsdl §3.2, §7). They hold for
    /// a component no system lists, here in a workspace with no system, which
    /// has no closure (rsdl §3.1).
    #[test]
    fn a_component_line_names_one_service_or_one_interface() {
        let cases: &[(&str, &[&str])] = &[
            (
                "component C { offers veh.adas.cruise, requires LaneAssist, requires veh.diag.access }",
                &[],
            ),
            ("component C { requires veh.adas.LaneAssist }", &[]),
            (
                "component C { offers veh.adas.cruise, offers veh.adas.cruise }",
                &["RSDL-309"],
            ),
            (
                "component C { requires LaneAssist, requires veh.adas.LaneAssist }",
                &["RSDL-309"],
            ),
            ("component C { offers Lane }", &["RSDL-310"]),
            ("component C { offers veh.adas.LaneAssist }", &["RSDL-310"]),
            ("component C { offers veh.adas.brake }", &["RSDL-310"]),
            ("component C { requires veh.adas.cruise }", &["RSDL-311"]),
            ("component C { requires Brake }", &["RSDL-312"]),
            ("component C { requires veh.common.Speed }", &["RSDL-312"]),
            ("component C { requires veh.adas.brake }", &["RSDL-312"]),
            ("component C { requires C.primary }", &["RSDL-312"]),
            ("component C { offers C.Unit }", &["RSDL-307"]),
            (
                "component C [ instances = (primary, primary) ] {}",
                &["RSDL-306"],
            ),
            (
                "component C [ instances = (primary, Unit) ] {}",
                &["RSDL-307"],
            ),
        ];
        for (decl, expected) in cases {
            let text = format!("package veh.topology\nimport veh.adas.LaneAssist\n{decl}\n");
            let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
            assert_eq!(codes(&system), *expected, "`{decl}`");
            assert_eq!(system.closure, None, "`{decl}`: no system, no closure");
        }

        // RSDL-311 lists the service's interfaces.
        let system = check_topology(&[(
            "veh/topology/x.rsdl",
            "package veh.topology\ncomponent C { requires veh.adas.cruise }\n",
        )]);
        assert!(
            system.diagnostics[0]
                .message
                .contains("it lists `veh.adas.CruiseControl`"),
            "got: {}",
            system.diagnostics[0].message
        );
    }

    /// A component of another package is named by its qualified name or through
    /// an import (rsdl §2, §4).
    #[test]
    fn a_system_names_a_component_of_another_package() {
        let gateway: &[(&str, &str)] = &[(
            "veh/gateway/gateway.rsdl",
            "package veh.gateway\ncomponent Relay {}\ncomponent Bridge {}\n",
        )];
        let topology: &[(&str, &str)] = &[(
            "veh/topology/x.rsdl",
            "package veh.topology\nimport veh.gateway.Bridge\nsystem Vehicle { veh.gateway.Relay, Bridge }\n",
        )];
        let system = check(&[("veh.gateway", gateway), ("veh.topology", topology)]);
        assert_eq!(codes(&system), Vec::<&str>::new());
        let ids: Vec<ComponentId> = system
            .closure
            .expect("the workspace declares a system")
            .components
            .into_iter()
            .map(|component| component.id)
            .collect();
        assert_eq!(
            ids,
            [
                ComponentId::Declared {
                    package: "veh.gateway".to_string(),
                    name: "Relay".to_string()
                },
                ComponentId::Declared {
                    package: "veh.gateway".to_string(),
                    name: "Bridge".to_string()
                },
            ]
        );
    }

    /// The names of the rsdl declarations are in their package's namespace
    /// beside the typl and ridl declarations (rsdl §3): a second declaration of
    /// one name is TYPL-009, except two deployments (RSDL-708) and two typl or
    /// ridl declarations (the resolver's TYPL-009). A machine name is scoped to
    /// its deployment.
    #[test]
    fn a_declaration_name_is_unique_in_its_package() {
        type Files<'a> = &'a [(&'a str, &'a str)];
        let cases: &[(Files, &[&str])] = &[
            (
                &[("p/x.rsdl", "package p\ncomponent A {}\ncomponent A {}\n")],
                &["TYPL-009"],
            ),
            (
                &[("p/x.rsdl", "package p\ncomponent A {}\ndistribution A {}\n")],
                &["TYPL-009"],
            ),
            (
                &[(
                    "p/x.rsdl",
                    "package p\nsystem S {}\ndeployment S for S {}\n",
                )],
                &["TYPL-009"],
            ),
            (
                &[(
                    "p/x.rsdl",
                    "package p\nsystem S {}\ndeployment D for S {}\ndeployment D for S {}\n",
                )],
                // Two deployments of one name are RSDL-708, not TYPL-009 (rsdl
                // reference §3.4); this query suppresses TYPL-009 for the pair.
                &["RSDL-708"],
            ),
            (
                &[
                    ("p/a.typl", "package p\ntype A: boolean\n"),
                    ("p/b.rsdl", "package p\ncomponent A {}\n"),
                ],
                &["TYPL-009"],
            ),
            (
                &[
                    ("p/a.rsdl", "package p\ncomponent A {}\n"),
                    ("p/b.ridl", "package p\ninterface A {}\n"),
                ],
                &["TYPL-009"],
            ),
            // The resolver reports two typl or ridl declarations; this query does not.
            (
                &[("p/a.typl", "package p\ntype A: boolean\ntype A: boolean\n")],
                &[],
            ),
            (
                &[(
                    "p/x.rsdl",
                    "package p\nsystem S {}\ndeployment D for S {\n  machine M {}\n  machine M {}\n}\n",
                )],
                // A machine name is scoped to its deployment, not the package
                // namespace (rsdl reference §3.5), so this query draws no
                // TYPL-009; two machines of one name are RSDL-705.
                &["RSDL-705"],
            ),
            (
                &[(
                    "p/x.rsdl",
                    "package p\ncomponent M {}\nsystem S {}\ndeployment D for S { machine M {} }\n",
                )],
                &[],
            ),
        ];
        for (files, expected) in cases {
            let system = check(&[("p", files)]);
            assert_eq!(codes(&system), *expected, "{files:?}");
        }

        // The same name in two packages is two names.
        let first: &[(&str, &str)] = &[("p/x.rsdl", "package p\ncomponent A {}\n")];
        let second: &[(&str, &str)] = &[("q/x.rsdl", "package q\ncomponent A {}\n")];
        assert_eq!(
            codes(&check(&[("p", first), ("q", second)])),
            Vec::<&str>::new()
        );

        // The later declaration is reported, with the resolver's message, even
        // when it is the ridl one.
        let files: &[(&str, &str)] = &[
            ("p/a.rsdl", "package p\ncomponent A {}\n"),
            ("p/b.ridl", "package p\ninterface A {}\n"),
        ];
        let system = check(&[("p", files)]);
        let [duplicate] = system.diagnostics.as_slice() else {
            panic!("one diagnostic, got {:?}", system.diagnostics);
        };
        assert_eq!(duplicate.message, "duplicate declaration of `A`");
        let mut sources = SourceMap::new();
        for (path, text) in files {
            sources.file_id(path, text);
        }
        assert_eq!(sources.path(duplicate.primary.file), Some("p/b.ridl"));
    }

    /// Appendix A resolves every `requires` line of its closure (rsdl §8), and
    /// warns twice: for `Panel`'s and `Backend`'s `requires CruiseControl`, which
    /// resolve to `Cruise`, a redundant provider set (the "Warnings" item after
    /// the example).
    #[test]
    fn appendix_a_resolves_every_requires_line() {
        let system = check_topology(&[("veh/topology/system.rsdl", SYSTEM)]);
        assert_eq!(codes(&system), ["RSDL-409", "RSDL-409"]);
        let closure = system
            .closure
            .as_ref()
            .expect("Appendix A declares a system");
        let resolved: Vec<(&str, String, &str, &str)> = closure
            .requires
            .iter()
            .map(|require| {
                (
                    closure.components[require.consumer].id.text(),
                    require.interface.text(),
                    require.service.as_str(),
                    closure.components[require.producer].id.text(),
                )
            })
            .collect();
        let expected = [
            ("Cruise", "veh.adas.LaneAssist", "veh.adas.lane", "Lane"),
            (
                "Panel",
                "veh.adas.CruiseControl",
                "veh.adas.cruise",
                "Cruise",
            ),
            ("Panel", "veh.adas.LaneAssist", "veh.adas.lane", "Lane"),
            (
                "Backend",
                "veh.adas.CruiseControl",
                "veh.adas.cruise",
                "Cruise",
            ),
            (
                "Backend",
                "veh.diag.access",
                "veh.diag.access",
                "veh.diag.access",
            ),
        ];
        assert_eq!(
            resolved,
            expected.map(|(consumer, interface, service, producer)| {
                (consumer, interface.to_string(), service, producer)
            })
        );
    }

    /// The resolution rules (rsdl §3.2, §7, §8), one input per rule.
    #[test]
    fn a_requires_line_resolves_to_one_service_and_one_offerer() {
        let body: &[(&str, &str)] = &[(
            "veh/body/body.ridl",
            "package veh.body\n\
             interface Doors {\n  signal locked: boolean @[100ms..1s]\n}\n\
             interface Horn {\n  signal sounding: boolean @[100ms..1s]\n}\n\
             service veh.body.doors : Doors\n\
             service veh.body.horn : Horn\n\
             service veh.body.cabin : Doors, Horn\n\
             service veh.body.twin : Doors, Horn\n",
        )];
        let cases: &[(&str, &[&str])] = &[
            // One owning service, one offering component.
            (
                "component D { offers veh.body.doors }\ncomponent U { requires veh.body.Doors }\nsystem S { D, U }",
                &[],
            ),
            // A lone service stands for its offering component (rsdl §6).
            (
                "component U { requires veh.body.Horn }\nsystem S { U, veh.body.horn }",
                &[],
            ),
            // RSDL-502: two closure components offer one service.
            (
                "component D { offers veh.body.doors }\ncomponent E { offers veh.body.doors }\nsystem S { D, E }",
                &["RSDL-502"],
            ),
            // RSDL-408: one interface, two closure services, required or not.
            (
                "component D { offers veh.body.doors }\ncomponent C { offers veh.body.cabin }\nsystem S { D, C }",
                &["RSDL-408"],
            ),
            // RSDL-408 for every interface the two services list.
            (
                "component C { offers veh.body.cabin }\ncomponent T { offers veh.body.twin }\nsystem S { C, T }",
                &["RSDL-408", "RSDL-408"],
            ),
            // RSDL-403: no closure service lists `Horn`.
            (
                "component D { offers veh.body.doors }\ncomponent U { requires veh.body.Horn }\nsystem S { D, U }",
                &["RSDL-403"],
            ),
            // RSDL-308, a rule of one component: it holds with no system.
            (
                "component H { offers veh.body.horn, requires veh.body.Horn }",
                &["RSDL-308"],
            ),
            // RSDL-409, a warning: the offering component has two instances.
            (
                "component H [ instances = (left, right) ] { offers veh.body.horn }\n\
                 component U { requires veh.body.Horn }\nsystem S { H, U }",
                &["RSDL-409"],
            ),
        ];
        for (decls, expected) in cases {
            let text = format!("package veh.topology\n{decls}\n");
            let topology: &[(&str, &str)] = &[("veh/topology/x.rsdl", text.as_str())];
            let system = check(&[("veh.body", body), ("veh.topology", topology)]);
            assert_eq!(codes(&system), *expected, "`{decls}`");
        }

        // RSDL-403 names a component outside the closure that offers a service
        // listing the interface.
        let topology: &[(&str, &str)] = &[(
            "veh/topology/x.rsdl",
            "package veh.topology\ncomponent H { offers veh.body.horn }\n\
             component U { requires veh.body.Horn }\nsystem S { U }\n",
        )];
        let system = check(&[("veh.body", body), ("veh.topology", topology)]);
        let [missing] = system.diagnostics.as_slice() else {
            panic!("one diagnostic, got {:?}", system.diagnostics);
        };
        assert!(
            missing.message.contains("`H` offers `veh.body.horn`"),
            "got: {}",
            missing.message
        );
        assert_eq!(missing.severity, Severity::Error);
    }

    /// rsdl reference Appendix A, `bench.rsdl`, with the same package.
    pub(super) const BENCH: &str = r#"package veh.topology

deployment Bench for Vehicle {
  machine DevBox { Cruise, Lane, Panel, veh.diag.access, Backend }
}
"#;

    /// Appendix A's two deployments place every closure instance exactly once
    /// (rsdl §9), and a placement line's backend keys apply to the placement it
    /// makes (rsdl §13, the attribute map).
    #[test]
    fn appendix_a_places_every_instance_once_per_deployment() {
        let system = check_topology(&[
            ("veh/topology/system.rsdl", SYSTEM),
            ("veh/topology/production.rsdl", PRODUCTION),
            ("veh/topology/bench.rsdl", BENCH),
        ]);
        assert_eq!(errors(&system), Vec::<&str>::new());
        assert!(!system.closure_has_errors);
        let closure = system
            .closure
            .as_ref()
            .expect("Appendix A declares a system");
        let placed = |index: usize| -> Vec<(String, &str)> {
            system.placements[index]
                .placements
                .iter()
                .map(|placement| {
                    let component = closure.components[placement.component].id.text();
                    let machine = &system.deployments[index].machines[placement.machine];
                    (
                        format!("{component}.{}", placement.instance),
                        machine.name.name.as_str(),
                    )
                })
                .collect()
        };
        let production = [
            ("Cruise.primary", "AdasHpc"),
            ("Lane.Unit", "AdasHpc"),
            ("veh.diag.access.Unit", "AdasHpc"),
            ("Cruise.backup", "Cockpit"),
            ("Panel.Unit", "Cockpit"),
            ("Backend.Unit", "Cloud"),
        ];
        assert_eq!(
            placed(0),
            production.map(|(instance, machine)| (instance.to_string(), machine))
        );
        let bench = [
            ("Cruise.primary", "DevBox"),
            ("Cruise.backup", "DevBox"),
            ("Lane.Unit", "DevBox"),
            ("Panel.Unit", "DevBox"),
            ("veh.diag.access.Unit", "DevBox"),
            ("Backend.Unit", "DevBox"),
        ];
        assert_eq!(
            placed(1),
            bench.map(|(instance, machine)| (instance.to_string(), machine))
        );
        assert!(
            system
                .placements
                .iter()
                .all(|placement| !placement.has_errors)
        );

        let panel = &system.placements[0].placements[4];
        let line = &system.deployments[0].machines[panel.machine].members[panel.line];
        assert_eq!(line.reference.text(), "Panel");
        assert_eq!(line.backend_keys[0].namespace, "linux");
    }

    /// The deployment, machine and placement rules (rsdl §3.4, §3.5, §7, §9),
    /// one input per rule.
    #[test]
    fn a_deployment_places_every_closure_instance_exactly_once() {
        const CLOSURE: &str = "component Cruise [ instances = (primary, backup) ] { offers veh.adas.cruise }\n\
                               component Backend [ external ] { requires veh.diag.access }\n\
                               component Spare {}\n\
                               system Vehicle { Cruise, Backend, veh.diag.access }\n";
        const PLACED: &str = "Cruise, veh.diag.access, Backend";
        let cases: &[(String, &[&str])] = &[
            // An external component on an on-board machine is a stub (rsdl §9).
            (format!("deployment P for Vehicle {{ machine A {{ {PLACED} }} }}"), &[]),
            (
                "deployment P for veh.topology.Vehicle {\n  machine A { Cruise.primary, Cruise.backup, veh.diag.access }\n  machine Cloud [ external ] { Backend }\n}".to_string(),
                &[],
            ),
            ("deployment P for Nowhere {}".to_string(), &["RSDL-704"]),
            (
                format!("deployment P for Vehicle {{ machine A {{ {PLACED} }} }}\ndeployment P for Vehicle {{ machine A {{ {PLACED} }} }}"),
                &["RSDL-708"],
            ),
            (
                "deployment P for Vehicle {\n  machine A { Cruise, veh.diag.access }\n  machine A { Backend }\n}".to_string(),
                &["RSDL-705"],
            ),
            (format!("deployment P for Vehicle {{ machine A {{ {PLACED}, Spare }} }}"), &["RSDL-702"]),
            (format!("deployment P for Vehicle {{ machine A {{ {PLACED}, Nothing }} }}"), &["RSDL-702"]),
            (format!("deployment P for Vehicle {{ machine A {{ {PLACED}, Cruise.spare }} }}"), &["RSDL-702"]),
            (
                "deployment P for Vehicle { machine A { Cruise.primary, veh.diag.access, Backend } }".to_string(),
                &["RSDL-701"],
            ),
            (format!("deployment P for Vehicle {{ machine A {{ {PLACED}, Cruise.primary }} }}"), &["RSDL-706"]),
            (
                "deployment P for Vehicle {\n  machine A { Cruise.primary, veh.diag.access, Backend }\n  machine B { Cruise }\n}".to_string(),
                &["RSDL-706"],
            ),
            (
                "deployment P for Vehicle {\n  machine A { Backend }\n  machine Cloud [ external ] { Cruise, veh.diag.access }\n}".to_string(),
                &["RSDL-707", "RSDL-707"],
            ),
            (format!("deployment P for Vehicle {{ machine A {{ {PLACED}, Cruise.Unit }} }}"), &["RSDL-307"]),
            (format!("deployment P for Vehicle {{ machine A {{ {PLACED}, veh.adas.cruise }} }}"), &["RSDL-504"]),
        ];
        for (deployments, expected) in cases {
            let text = format!("package veh.topology\n{CLOSURE}{deployments}\n");
            let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
            assert_eq!(codes(&system), *expected, "`{deployments}`");
        }
    }

    /// rsdl §13: an RSDL-7xx error blocks its own deployment, and any other
    /// error blocks every deployment.
    #[test]
    fn a_placement_error_blocks_its_own_deployment_only() {
        let text = "package veh.topology\n\
                    component Lane { offers veh.adas.lane }\n\
                    system Vehicle { Lane }\n\
                    deployment Good for Vehicle { machine A { Lane } }\n\
                    deployment Bad for Vehicle { machine A {} }\n";
        let system = check_topology(&[("veh/topology/x.rsdl", text)]);
        assert_eq!(codes(&system), ["RSDL-701"]);
        assert!(!system.closure_has_errors);
        let blocked: Vec<bool> = system.placements.iter().map(|p| p.has_errors).collect();
        assert_eq!(blocked, [false, true]);

        let text = text.replace("system Vehicle { Lane }", "system Vehicle { Lane, Lane }");
        let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
        assert_eq!(codes(&system), ["RSDL-603", "RSDL-701"]);
        assert!(system.closure_has_errors);
    }

    /// The closure the sizing-key tests deploy: `Panel` consumes an event
    /// channel, `Backend` a query channel, `Lane` nothing.
    const SIZED: &str = "package veh.topology\n\
                         import veh.adas.LaneAssist\n\
                         component Lane { offers veh.adas.lane }\n\
                         component Panel { requires LaneAssist }\n\
                         component Backend [ external ] { requires veh.diag.access }\n\
                         system Vehicle { Lane, Panel, Backend, veh.diag.access }\n";

    /// Every instance of `SIZED` on one machine.
    const SIZED_LINES: &str = "Lane, Panel, Backend, veh.diag.access";

    const UNSIZED: Sizing = Sizing {
        depth: None,
        slots: None,
        budget: None,
    };

    /// rsdl §5: `depth`, `slots` and `budget` are read on a `deployment` and on
    /// a placement line, each key at both ends of its range.
    #[test]
    fn the_sizing_keys_are_read_on_a_deployment_and_a_placement_line() {
        let text = format!(
            "{SIZED}\
             deployment Prod for Vehicle \
             [ depth = 1, slots = 65536, budget = 18446744073709551615 ] {{\n  \
             machine A {{ Lane, Panel [ depth = 4294967295, slots = 1, budget = 1 ], \
             Backend, veh.diag.access }}\n}}\n"
        );
        let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
        assert_eq!(codes(&system), Vec::<&str>::new());
        let deployment = &system.deployments[0];
        assert_eq!(
            deployment.sizing,
            Sizing {
                depth: Some(1),
                slots: Some(65536),
                budget: Some(18446744073709551615),
            }
        );
        let lines: Vec<Sizing> = deployment.machines[0]
            .members
            .iter()
            .map(|member| member.sizing)
            .collect();
        assert_eq!(
            lines,
            [
                UNSIZED,
                Sizing {
                    depth: Some(4294967295),
                    slots: Some(1),
                    budget: Some(1),
                },
                UNSIZED,
                UNSIZED,
            ]
        );
        assert!(!system.placements[0].has_errors);
    }

    /// rsdl §5, §13: a value that is not an integer within the key's range is
    /// RSDL-709, which blocks its own deployment and no other, and writes
    /// nothing.
    #[test]
    fn a_sizing_key_out_of_range_is_rsdl_709_and_blocks_its_deployment() {
        let cases: &[(&str, &str)] = &[
            (
                "slots = 0",
                "`slots` takes an integer from 1 to 65536, written `slots = <n>`; `0` is not one \
                 (rsdl reference §5)",
            ),
            (
                "slots = 65537",
                "`slots` takes an integer from 1 to 65536, written `slots = <n>`; `65537` is not \
                 one (rsdl reference §5)",
            ),
            (
                "budget = 0",
                "`budget` takes an integer from 1 to 18446744073709551615, written \
                 `budget = <n>`; `0` is not one (rsdl reference §5)",
            ),
            (
                "budget = 18446744073709551616",
                "`budget` takes an integer from 1 to 18446744073709551615, written \
                 `budget = <n>`; `18446744073709551616` is not one (rsdl reference §5)",
            ),
            (
                "depth = 0",
                "`depth` takes an integer from 1 to 4294967295, written `depth = <n>`; `0` is \
                 not one (rsdl reference §5)",
            ),
            (
                "depth = 4294967296",
                "`depth` takes an integer from 1 to 4294967295, written `depth = <n>`; \
                 `4294967296` is not one (rsdl reference §5)",
            ),
            (
                "depth = -1",
                "`depth` takes an integer from 1 to 4294967295, written `depth = <n>`; `-1` is \
                 not one (rsdl reference §5)",
            ),
            (
                "slots = (1, 2)",
                "`slots` takes an integer from 1 to 65536, written `slots = <n>`; `(1,2)` is \
                 not one (rsdl reference §5)",
            ),
            (
                "slots = \"8\"",
                "`slots` takes an integer from 1 to 65536, written `slots = <n>`; `\"8\"` is \
                 not one (rsdl reference §5)",
            ),
            (
                "slots",
                "`slots` takes an integer from 1 to 65536, written `slots = <n>`; a bare \
                 `slots` is not one (rsdl reference §5)",
            ),
        ];
        for (attribute, message) in cases {
            let sites = [
                format!(
                    "deployment Bad for Vehicle [ {attribute} ] {{ machine A {{ {SIZED_LINES} }} }}"
                ),
                format!(
                    "deployment Bad for Vehicle {{ machine A {{ Lane, Panel [ {attribute} ], \
                     Backend, veh.diag.access }} }}"
                ),
            ];
            for bad in sites {
                let good =
                    format!("deployment Good for Vehicle {{ machine A {{ {SIZED_LINES} }} }}");
                // `Bad` after `Good`, then before it: the block follows the
                // deployment, not the order of declaration.
                let orders = [
                    (format!("{SIZED}{good}\n{bad}\n"), 1, [false, true]),
                    (format!("{SIZED}{bad}\n{good}\n"), 0, [true, false]),
                ];
                for (text, bad_index, expected) in orders {
                    let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
                    assert_eq!(codes(&system), ["RSDL-709"], "`{bad}`");
                    assert_eq!(system.diagnostics[0].message, *message, "`{bad}`");
                    assert!(!system.closure_has_errors, "`{bad}`");
                    let blocked: Vec<bool> =
                        system.placements.iter().map(|p| p.has_errors).collect();
                    assert_eq!(blocked, expected, "`{bad}`");
                    let deployment = &system.deployments[bad_index];
                    assert_eq!(deployment.sizing, UNSIZED, "`{bad}`");
                    let lines: Vec<Sizing> = deployment.machines[0]
                        .members
                        .iter()
                        .map(|member| member.sizing)
                        .collect();
                    assert_eq!(lines, [UNSIZED; 4], "`{bad}`");
                }
            }
        }

        // `slots = 50ms`: the parser refuses a duration as an attribute value
        // and reports FORM-101, a parse error the rsdl reporter never sees,
        // which stops `ridl build` writing anything. The reader draws no
        // RSDL-709 over it: a second report would call `slots` bare.
        let text = format!(
            "{SIZED}deployment Prod for Vehicle [ slots = 50ms ] {{ machine A {{ {SIZED_LINES} }} }}\n"
        );
        let parsed = ridl_syntax::parse(&text, Profile::Rsdl);
        let parse_codes: Vec<&str> = parsed.errors().iter().map(|error| error.code).collect();
        assert_eq!(parse_codes, ["FORM-101"]);
        let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
        assert_eq!(codes(&system), Vec::<&str>::new());
        assert_eq!(system.deployments[0].sizing, UNSIZED);
    }

    /// rsdl §5, §13: the sizing keys are legal on a `deployment` and on a
    /// placement line only; anywhere else, FORM-107, which blocks every
    /// deployment through `closure_has_errors` and marks none on its own.
    #[test]
    fn a_sizing_key_on_a_requires_line_or_a_machine_is_form_107() {
        let good = format!("deployment Prod for Vehicle {{ machine A {{ {SIZED_LINES} }} }}\n");
        let cases: [String; 8] = [
            format!(
                "{}{good}",
                SIZED.replace("requires LaneAssist", "requires LaneAssist [ depth = 2 ]")
            ),
            format!(
                "{SIZED}deployment Prod for Vehicle {{ machine A [ slots = 4 ] {{ {SIZED_LINES} }} }}\n"
            ),
            format!(
                "{}{good}",
                SIZED.replace(
                    "offers veh.adas.lane",
                    "offers veh.adas.lane [ budget = 1 ]"
                )
            ),
            format!(
                "{}{good}",
                SIZED.replace("component Lane {", "component Lane [ depth = 2 ] {")
            ),
            format!(
                "{}{good}",
                SIZED.replace("system Vehicle {", "system Vehicle [ slots = 4 ] {")
            ),
            format!(
                "{}{good}",
                SIZED.replace(
                    "system Vehicle { Lane,",
                    "system Vehicle { Lane [ budget = 1 ],"
                )
            ),
            format!(
                "{SIZED}{good}distribution D [ depth = 2 ] {{ Lane, Panel, veh.diag.access }}\n"
            ),
            format!(
                "{SIZED}{good}distribution D {{ Lane [ slots = 4 ], Panel, veh.diag.access }}\n"
            ),
        ];
        for text in &cases {
            let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
            assert_eq!(codes(&system), ["FORM-107"], "`{text}`");
            assert!(system.closure_has_errors, "`{text}`");
            assert!(!system.placements[0].has_errors, "`{text}`");
        }
        let system = check_topology(&[("veh/topology/x.rsdl", cases[1].as_str())]);
        assert_eq!(
            system.diagnostics[0].message,
            "attribute `slots` not valid on a `machine` (rsdl reference §5)"
        );
        let text = format!(
            "{SIZED}deployment Prod for Vehicle {{ machine A {{ Lane, Panel [ labels = (QM) ], \
             Backend, veh.diag.access }} }}\n"
        );
        let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
        assert_eq!(codes(&system), ["FORM-107"]);
        assert_eq!(
            system.diagnostics[0].message,
            "attribute `labels` not valid on a placement line — a placement line takes backend \
             keys and the sizing keys `depth`, `slots` and `budget` (rsdl reference §5)"
        );
    }

    /// rsdl §5: a placement line takes the sizing keys and backend keys; every
    /// other rsdl-owned key on one is FORM-107, with no fact read from it.
    #[test]
    fn another_rsdl_key_on_a_placement_line_is_form_107() {
        let cases = [
            "instances = (a)",
            "external",
            "tier = PLATFORM",
            "deprecated = \"x\"",
        ];
        for attribute in cases {
            let text = format!(
                "{SIZED}deployment Prod for Vehicle {{ machine A {{ Lane, Panel [ {attribute} ], \
                 Backend, veh.diag.access }} }}\n"
            );
            let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
            assert_eq!(codes(&system), ["FORM-107"], "`{attribute}`");
            assert!(system.closure_has_errors, "`{attribute}`");
        }
    }

    /// rsdl §5, §13: a sizing key twice in one block is FORM-108; the first
    /// value stands, and the error blocks through `closure_has_errors`, not
    /// through the deployment's own flag.
    #[test]
    fn a_sizing_key_twice_in_one_block_is_form_108() {
        let text = format!(
            "{SIZED}deployment Prod for Vehicle [ depth = 2, depth = 3 ] {{ machine A {{ \
             {SIZED_LINES} }} }}\n"
        );
        let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
        assert_eq!(codes(&system), ["FORM-108"]);
        assert_eq!(system.deployments[0].sizing.depth, Some(2));
        assert!(system.closure_has_errors);
        assert!(!system.placements[0].has_errors);

        let text = format!(
            "{SIZED}deployment Prod for Vehicle {{ machine A {{ Lane, Panel [ slots = 4, \
             slots = 5 ], Backend, veh.diag.access }} }}\n"
        );
        let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
        assert_eq!(codes(&system), ["FORM-108"]);
        assert_eq!(
            system.deployments[0].machines[0].members[1].sizing.slots,
            Some(4)
        );
        assert!(system.closure_has_errors);
        assert!(!system.placements[0].has_errors);
    }

    /// rsdl §5: a sizing key on an instance that consumes no channel of the
    /// matching kind draws nothing; the value is still read.
    #[test]
    fn a_sizing_key_on_an_instance_that_consumes_nothing_draws_nothing() {
        let text = format!(
            "{SIZED}deployment Prod for Vehicle {{ machine A {{ \
             Lane [ depth = 2, slots = 4, budget = 1024 ], Panel, Backend, veh.diag.access }} }}\n"
        );
        let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
        assert_eq!(codes(&system), Vec::<&str>::new());
        assert_eq!(
            system.deployments[0].machines[0].members[0].sizing,
            Sizing {
                depth: Some(2),
                slots: Some(4),
                budget: Some(1024),
            }
        );
        assert!(!system.placements[0].has_errors);
    }

    /// The contract package of the depth tests. The contract bound of each
    /// event, `ceil(max / min)`: `Doors.opened` 10, `Trunk.opened` 10,
    /// `Trunk.closed` 5, `Hood.raised` 4 (3.33 rounded up); `Latch.jammed`
    /// and `Spare.idle` are half-open and have none; `Clock.tick` is untimed,
    /// so the §9.1 defaults give it both bounds.
    const EVENTS: &str = "package veh.ev\n\
                          interface Doors {\n  event opened: boolean @[100ms..1s]\n}\n\
                          interface Trunk {\n  event opened: boolean @[100ms..1s]\n  \
                          event closed: boolean @[100ms..500ms]\n}\n\
                          interface Hood {\n  event raised: boolean @[300ms..1s]\n}\n\
                          interface Latch {\n  event jammed: boolean @[100ms..]\n}\n\
                          interface Spare {\n  event idle: boolean @[100ms..]\n}\n\
                          interface Clock {\n  event tick: boolean\n}\n\
                          service veh.ev.doors : Doors\n\
                          service veh.ev.trunk : Trunk\n\
                          service veh.ev.hood : Hood\n\
                          service veh.ev.latch : Latch\n\
                          service veh.ev.spare : Spare\n\
                          service veh.ev.clock : Clock\n";

    /// The components of the depth tests: `Body` offers every service of
    /// `EVENTS`, each consumer requires one interface, and `Cloud` and
    /// `Remote` are the external ends of a `Latch` link. A test lists in its
    /// `system` the components it needs, so `Body` and `Cloud` never offer
    /// `veh.ev.latch` in one closure.
    const EV_TOPOLOGY: &str = "package veh.topology\n\
                               import veh.ev.Doors\n\
                               import veh.ev.Trunk\n\
                               import veh.ev.Hood\n\
                               import veh.ev.Latch\n\
                               import veh.ev.Clock\n\
                               component Body {\n  offers veh.ev.doors\n  offers veh.ev.trunk\n  \
                               offers veh.ev.hood\n  offers veh.ev.latch\n  offers veh.ev.spare\n  \
                               offers veh.ev.clock\n}\n\
                               component Panel { requires Doors }\n\
                               component Mirror { requires Doors }\n\
                               component Pair [ instances = (a, b) ] { requires Doors }\n\
                               component Hatch { requires Trunk }\n\
                               component Bonnet { requires Hood }\n\
                               component Lock { requires Latch }\n\
                               component Dash { requires Clock }\n\
                               component Remote [ external ] { requires Latch }\n\
                               component Cloud [ external ] { offers veh.ev.latch }\n";

    /// Checks `EVENTS` beside `EV_TOPOLOGY` followed by `topology`, the
    /// system and deployment of one test.
    fn check_events(topology: &str) -> CheckedSystem {
        let ev: &[(&str, &str)] = &[("veh/ev/ev.ridl", EVENTS)];
        let text = format!("{EV_TOPOLOGY}{topology}");
        let topology: &[(&str, &str)] = &[("veh/topology/x.rsdl", text.as_str())];
        check(&[("veh.ev", ev), ("veh.topology", topology)])
    }

    fn messages(system: &CheckedSystem) -> Vec<&str> {
        system
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect()
    }

    /// The source text under each diagnostic's primary span, for the file
    /// `check_events` builds from `topology`.
    fn spans<'a>(system: &CheckedSystem, text: &'a str) -> Vec<&'a str> {
        system
            .diagnostics
            .iter()
            .map(|d| {
                &text[usize::from(d.primary.range.start())..usize::from(d.primary.range.end())]
            })
            .collect()
    }

    /// rsdl §5: a declared `depth` below the contract bound of an event a
    /// covered link consumes is RSDL-805, once per link and event, naming
    /// the event, the declared value and the bound.
    #[test]
    fn a_declared_depth_below_the_bound_is_rsdl_805_once_per_link_and_event() {
        let system = check_events(
            "system Vehicle { Body, Panel, Mirror, Hatch }\n\
             deployment Prod for Vehicle { machine A { \
             Body, Panel [ depth = 2 ], Mirror [ depth = 3 ], Hatch [ depth = 4 ] } }\n",
        );
        assert_eq!(
            codes(&system),
            ["RSDL-805", "RSDL-805", "RSDL-805", "RSDL-805"]
        );
        assert_eq!(
            messages(&system),
            [
                "`depth = 2` on `Panel` is below the contract bound 10 of `veh.ev.Doors.opened` — \
                 the bound is `ceil(max / min)` over the event's timing, and a ring below it can \
                 drop occurrences alive at once (rsdl reference §5)",
                "`depth = 3` on `Mirror` is below the contract bound 10 of `veh.ev.Doors.opened` — \
                 the bound is `ceil(max / min)` over the event's timing, and a ring below it can \
                 drop occurrences alive at once (rsdl reference §5)",
                "`depth = 4` on `Hatch` is below the contract bound 10 of `veh.ev.Trunk.opened` — \
                 the bound is `ceil(max / min)` over the event's timing, and a ring below it can \
                 drop occurrences alive at once (rsdl reference §5)",
                "`depth = 4` on `Hatch` is below the contract bound 5 of `veh.ev.Trunk.closed` — \
                 the bound is `ceil(max / min)` over the event's timing, and a ring below it can \
                 drop occurrences alive at once (rsdl reference §5)",
            ]
        );
        assert!(
            system
                .diagnostics
                .iter()
                .all(|d| d.severity == Severity::Warning),
            "{:?}",
            system.diagnostics
        );
    }

    /// rsdl §5: a `depth` exactly at the bound draws nothing, and one exactly
    /// one below it draws RSDL-805.
    #[test]
    fn a_depth_one_below_the_bound_is_rsdl_805_and_one_at_the_bound_is_not() {
        let below = check_events(
            "system Vehicle { Body, Panel }\n\
             deployment Prod for Vehicle { machine A { Body, Panel [ depth = 9 ] } }\n",
        );
        assert_eq!(codes(&below), ["RSDL-805"]);
        assert_eq!(
            messages(&below),
            [
                "`depth = 9` on `Panel` is below the contract bound 10 of `veh.ev.Doors.opened` — \
              the bound is `ceil(max / min)` over the event's timing, and a ring below it can \
              drop occurrences alive at once (rsdl reference §5)"
            ]
        );
        let at = check_events(
            "system Vehicle { Body, Panel }\n\
             deployment Prod for Vehicle { machine A { Body, Panel [ depth = 10 ] } }\n",
        );
        assert_eq!(codes(&at), Vec::<&str>::new());
    }

    /// ADR-0015 decision 21: the bound is rounded up. `Hood.raised` is
    /// `@[300ms..1s]`, 3.33, so its bound is 4 and `depth = 3` is below it.
    #[test]
    fn a_rounded_up_bound_is_reported_rounded_up() {
        let system = check_events(
            "system Vehicle { Body, Bonnet }\n\
             deployment Prod for Vehicle { machine A { Body, Bonnet [ depth = 3 ] } }\n",
        );
        assert_eq!(
            messages(&system),
            [
                "`depth = 3` on `Bonnet` is below the contract bound 4 of `veh.ev.Hood.raised` — \
              the bound is `ceil(max / min)` over the event's timing, and a ring below it can \
              drop occurrences alive at once (rsdl reference §5)"
            ]
        );
        let at = check_events(
            "system Vehicle { Body, Bonnet }\n\
             deployment Prod for Vehicle { machine A { Body, Bonnet [ depth = 4 ] } }\n",
        );
        assert_eq!(codes(&at), Vec::<&str>::new());
    }

    /// rsdl §5: a declared `depth` at or above the bound is accepted silently.
    #[test]
    fn a_depth_at_or_above_the_bound_draws_nothing() {
        for depth in ["10", "11", "4294967295"] {
            let system = check_events(&format!(
                "system Vehicle {{ Body, Panel, Hatch }}\n\
                 deployment Prod for Vehicle {{ machine A {{ \
                 Body, Panel [ depth = {depth} ], Hatch [ depth = {depth} ] }} }}\n"
            ));
            assert_eq!(codes(&system), Vec::<&str>::new(), "depth = {depth}");
        }
    }

    /// rsdl §5: a `depth` declared on the `deployment` reaches every covered
    /// link, and RSDL-805 is drawn once per link, not once for the
    /// deployment. `Lock`'s half-open event has no bound, and the declared
    /// value silences RSDL-806 for it.
    #[test]
    fn a_deployment_depth_below_the_bound_is_rsdl_805_once_per_covered_link() {
        let topology = "system Vehicle { Body, Panel, Mirror, Lock }\n\
                        deployment Prod for Vehicle [ depth = 2 ] { machine A { \
                        Body, Panel, Mirror, Lock } }\n";
        let system = check_events(topology);
        assert_eq!(codes(&system), ["RSDL-805", "RSDL-805"]);
        assert_eq!(
            messages(&system),
            [
                "`depth = 2` on `Panel` is below the contract bound 10 of `veh.ev.Doors.opened` — \
                 the bound is `ceil(max / min)` over the event's timing, and a ring below it can \
                 drop occurrences alive at once (rsdl reference §5)",
                "`depth = 2` on `Mirror` is below the contract bound 10 of `veh.ev.Doors.opened` — \
                 the bound is `ceil(max / min)` over the event's timing, and a ring below it can \
                 drop occurrences alive at once (rsdl reference §5)",
            ]
        );
        // The deployment declares the value, so the diagnostic points at the
        // deployment's name rather than at a placement line.
        let text = format!("{EV_TOPOLOGY}{topology}");
        assert_eq!(spans(&system, &text), ["Prod", "Prod"]);
    }

    /// rsdl §5: the placement line's value takes precedence over the
    /// deployment's, and the comparison reads the value that wins: a line
    /// value above the bound silences a deployment value below it, and a line
    /// value below the bound is reported under a deployment value above it.
    /// A line's RSDL-805 points at the `depth` attribute, as RSDL-709 does.
    #[test]
    fn a_placement_depth_takes_precedence_over_a_deployment_depth() {
        let line_above = check_events(
            "system Vehicle { Body, Panel, Mirror }\n\
             deployment Prod for Vehicle [ depth = 2 ] { machine A { \
             Body, Panel [ depth = 10 ], Mirror } }\n",
        );
        assert_eq!(
            messages(&line_above),
            [
                "`depth = 2` on `Mirror` is below the contract bound 10 of `veh.ev.Doors.opened` — \
              the bound is `ceil(max / min)` over the event's timing, and a ring below it can \
              drop occurrences alive at once (rsdl reference §5)"
            ]
        );
        let topology = "system Vehicle { Body, Panel, Mirror }\n\
                        deployment Prod for Vehicle [ depth = 10 ] { machine A { \
                        Body, Panel [ depth = 2 ], Mirror } }\n";
        let line_below = check_events(topology);
        assert_eq!(
            messages(&line_below),
            [
                "`depth = 2` on `Panel` is below the contract bound 10 of `veh.ev.Doors.opened` — \
              the bound is `ceil(max / min)` over the event's timing, and a ring below it can \
              drop occurrences alive at once (rsdl reference §5)"
            ]
        );
        let text = format!("{EV_TOPOLOGY}{topology}");
        assert_eq!(spans(&line_below, &text), ["depth = 2"]);
    }

    /// rsdl §5, §7: a consumer with two instances has two links, each read
    /// from its own placement line and named by its instance.
    #[test]
    fn a_multi_instance_consumer_is_read_line_by_line_and_named_per_instance() {
        let one_below = check_events(
            "system Vehicle { Body, Pair }\n\
             deployment Prod for Vehicle { machine A { \
             Body, Pair.a [ depth = 2 ], Pair.b [ depth = 20 ] } }\n",
        );
        assert_eq!(
            messages(&one_below),
            [
                "`depth = 2` on `Pair.a` is below the contract bound 10 of `veh.ev.Doors.opened` — \
              the bound is `ceil(max / min)` over the event's timing, and a ring below it can \
              drop occurrences alive at once (rsdl reference §5)"
            ]
        );
        let both_below = check_events(
            "system Vehicle { Body, Pair }\n\
             deployment Prod for Vehicle { machine A { \
             Body, Pair.a [ depth = 2 ], Pair.b [ depth = 3 ] } }\n",
        );
        assert_eq!(
            messages(&both_below),
            [
                "`depth = 2` on `Pair.a` is below the contract bound 10 of `veh.ev.Doors.opened` — \
                 the bound is `ceil(max / min)` over the event's timing, and a ring below it can \
                 drop occurrences alive at once (rsdl reference §5)",
                "`depth = 3` on `Pair.b` is below the contract bound 10 of `veh.ev.Doors.opened` — \
                 the bound is `ceil(max / min)` over the event's timing, and a ring below it can \
                 drop occurrences alive at once (rsdl reference §5)",
            ]
        );
    }

    /// rsdl §5: an event with an explicit half-open range consumed by a link
    /// with no declared `depth` is RSDL-806.
    #[test]
    fn a_half_open_event_with_no_declared_depth_is_rsdl_806() {
        let system = check_events(
            "system Vehicle { Body, Lock }\n\
             deployment Prod for Vehicle { machine A { Body, Lock } }\n",
        );
        assert_eq!(codes(&system), ["RSDL-806"]);
        assert_eq!(
            messages(&system),
            [
                "`Lock` declares no `depth` for `veh.ev.Latch.jammed`, whose timing is an explicit \
              half-open range — a link to an event with no derivable bound takes its ring depth \
              from `depth = <n>` on the placement line or on the deployment (rsdl reference §5)"
            ]
        );
        assert_eq!(system.diagnostics[0].severity, Severity::Warning);
    }

    /// rsdl §5: an event whose bounds are both written but whose ratio is
    /// above 4294967295 has no derivable bound either; RSDL-806 is drawn, and
    /// the message does not claim a half-open range.
    #[test]
    fn an_event_whose_ratio_exceeds_the_depth_range_is_rsdl_806() {
        let text = EVENTS.replace(
            "event jammed: boolean @[100ms..]",
            "event jammed: boolean @[1us..2h]",
        );
        let ev: &[(&str, &str)] = &[("veh/ev/ev.ridl", text.as_str())];
        let topology = format!(
            "{EV_TOPOLOGY}system Vehicle {{ Body, Lock }}\n\
             deployment Prod for Vehicle {{ machine A {{ Body, Lock }} }}\n"
        );
        let topology: &[(&str, &str)] = &[("veh/topology/x.rsdl", topology.as_str())];
        let system = check(&[("veh.ev", ev), ("veh.topology", topology)]);
        assert_eq!(codes(&system), ["RSDL-806"]);
        assert_eq!(
            messages(&system),
            [
                "`Lock` declares no `depth` for `veh.ev.Latch.jammed`, whose contract bound \
              `ceil(max / min)` is not derivable from its timing — a link to an event with no \
              derivable bound takes its ring depth from `depth = <n>` on the placement line or on \
              the deployment (rsdl reference §5)"
            ]
        );
    }

    /// rsdl §5: a declared `depth`, on the placement line or on the
    /// deployment, is the link's depth, so RSDL-806 is not drawn.
    #[test]
    fn a_declared_depth_silences_rsdl_806() {
        let on_line = check_events(
            "system Vehicle { Body, Lock }\n\
             deployment Prod for Vehicle { machine A { Body, Lock [ depth = 1 ] } }\n",
        );
        assert_eq!(codes(&on_line), Vec::<&str>::new());
        let on_deployment = check_events(
            "system Vehicle { Body, Lock }\n\
             deployment Prod for Vehicle [ depth = 1 ] { machine A { Body, Lock } }\n",
        );
        assert_eq!(codes(&on_deployment), Vec::<&str>::new());
    }

    /// rsdl §10: a `requires` whose two ends are external lowers no link, so
    /// the half-open event between them draws nothing.
    #[test]
    fn two_external_ends_lower_no_link_and_draw_neither_code() {
        let system = check_events(
            "system Vehicle { Cloud, Remote }\n\
             deployment Prod for Vehicle { machine X [ external ] { Cloud, Remote } }\n",
        );
        assert_eq!(codes(&system), Vec::<&str>::new());
    }

    /// rsdl §10: a link with one external end is lowered, read from the
    /// consumer's side — an external consumer of an internal producer draws
    /// RSDL-806 once.
    #[test]
    fn an_external_consumer_of_an_internal_producer_draws_rsdl_806() {
        let system = check_events(
            "system Vehicle { Body, Remote }\n\
             deployment Prod for Vehicle { \
             machine A { Body } machine X [ external ] { Remote } }\n",
        );
        assert_eq!(
            messages(&system),
            [
                "`Remote` declares no `depth` for `veh.ev.Latch.jammed`, whose timing is an explicit \
              half-open range — a link to an event with no derivable bound takes its ring depth \
              from `depth = <n>` on the placement line or on the deployment (rsdl reference §5)"
            ]
        );
    }

    /// rsdl §10: an internal consumer of an external producer draws RSDL-806
    /// once.
    #[test]
    fn an_internal_consumer_of_an_external_producer_draws_rsdl_806() {
        let system = check_events(
            "system Vehicle { Cloud, Lock }\n\
             deployment Prod for Vehicle { \
             machine A { Lock } machine X [ external ] { Cloud } }\n",
        );
        assert_eq!(
            messages(&system),
            [
                "`Lock` declares no `depth` for `veh.ev.Latch.jammed`, whose timing is an explicit \
              half-open range — a link to an event with no derivable bound takes its ring depth \
              from `depth = <n>` on the placement line or on the deployment (rsdl reference §5)"
            ]
        );
    }

    /// rsdl §5, ridl §9.1: an event written with no timing resolves the
    /// package default, so it has both bounds and draws neither code.
    #[test]
    fn an_event_with_no_timing_draws_neither_code() {
        let undeclared = check_events(
            "system Vehicle { Body, Dash }\n\
             deployment Prod for Vehicle { machine A { Body, Dash } }\n",
        );
        assert_eq!(codes(&undeclared), Vec::<&str>::new());
        let declared = check_events(
            "system Vehicle { Body, Dash }\n\
             deployment Prod for Vehicle { machine A { Body, Dash [ depth = 4294967295 ] } }\n",
        );
        assert_eq!(codes(&declared), Vec::<&str>::new());
    }

    /// rsdl §5: `depth` applies to event channels only; a placement that
    /// consumes a query channel and no event draws neither code.
    #[test]
    fn a_depth_on_a_placement_that_consumes_only_calls_draws_neither_code() {
        let text = format!(
            "{SIZED}deployment Prod for Vehicle {{ machine A {{ \
             Lane, Panel, Backend [ depth = 1 ], veh.diag.access }} }}\n"
        );
        let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
        assert_eq!(codes(&system), Vec::<&str>::new());
    }

    /// rsdl §5: the rule is per consumer link, so a half-open event no link
    /// consumes draws nothing — `Spare.idle` is offered by `Body` and
    /// required by nobody.
    #[test]
    fn a_half_open_event_that_no_link_consumes_draws_neither_code() {
        let system = check_events(
            "system Vehicle { Body, Panel }\n\
             deployment Prod for Vehicle { machine A { Body, Panel } }\n",
        );
        assert_eq!(codes(&system), Vec::<&str>::new());
    }

    /// rsdl §13: an RSDL-7xx error blocks its deployment, and its placement
    /// set is not one to read links from.
    #[test]
    fn a_blocked_deployment_draws_neither_code() {
        let system = check_events(
            "system Vehicle { Body, Panel, Lock }\n\
             deployment Prod for Vehicle [ slots = 0 ] { machine A { \
             Body, Panel [ depth = 2 ], Lock } }\n",
        );
        assert_eq!(codes(&system), ["RSDL-709"]);
    }

    /// rsdl §7: a redundant provider set lowers one link per producer
    /// instance, but the declared value and the bound belong to the consumer
    /// and the event, so RSDL-805 is drawn once per consumer instance.
    #[test]
    fn a_redundant_provider_set_draws_rsdl_805_once_per_consumer_instance() {
        let system = check_events(
            "component Twin [ instances = (a, b) ] { offers veh.ev.doors }\n\
             system Vehicle { Twin, Panel }\n\
             deployment Prod for Vehicle { machine A { Twin.a, Twin.b, Panel [ depth = 2 ] } }\n",
        );
        assert_eq!(codes(&system), ["RSDL-409", "RSDL-805"]);
    }

    /// Appendix A's distributions: every implemented closure component in one,
    /// `Backend` in none, and `Hmi` depends on `Adas` (rsdl §3.3, §13, the
    /// "Distributions" item after the example).
    #[test]
    fn appendix_a_derives_its_distribution_dependency() {
        let system = check_topology(&[("veh/topology/system.rsdl", SYSTEM)]);
        assert_eq!(errors(&system), Vec::<&str>::new());
        let closure = system
            .closure
            .as_ref()
            .expect("Appendix A declares a system");
        let facts = closure
            .distribution_facts
            .as_ref()
            .expect("Appendix A declares distributions");
        let membership: Vec<Option<&str>> = facts
            .membership
            .iter()
            .map(|held| held.map(|index| system.distributions[index].name.name.as_str()))
            .collect();
        assert_eq!(
            membership,
            [Some("Adas"), Some("Adas"), Some("Hmi"), None, Some("Adas")]
        );
        assert_eq!(
            facts.dependencies,
            [DistributionDependency { from: 1, to: 0 }]
        );
    }

    /// The distribution rules (rsdl §3.3), one input per rule.
    #[test]
    fn a_distribution_holds_each_implemented_closure_component_once() {
        const CLOSURE: &str = "component Lane { offers veh.adas.lane }\n\
                               component Panel { requires LaneAssist }\n\
                               component Backend [ external ] {}\n\
                               component Spare {}\n\
                               system Vehicle { Lane, Panel, Backend, veh.diag.access }\n";
        let cases: &[(&str, &[&str])] = &[
            ("distribution D { Lane, Panel, veh.diag.access }", &[]),
            // A workspace with no distribution derives no installation.
            ("", &[]),
            ("distribution D { Lane, Panel }", &["RSDL-904"]),
            (
                "distribution D { Lane, Panel, veh.diag.access, Spare }",
                &["RSDL-903"],
            ),
            (
                "distribution D { Lane, Panel, veh.diag.access, Nothing }",
                &["RSDL-903"],
            ),
            (
                "distribution D { Lane, Panel, veh.diag.access, Lane.Unit }",
                &["RSDL-307"],
            ),
            (
                "distribution D { Lane, Panel, veh.diag.access, veh.adas.lane }",
                &["RSDL-504"],
            ),
            (
                "distribution D { Lane, Panel, veh.diag.access, Lane }",
                &["RSDL-906"],
            ),
            (
                "distribution D { Lane, Panel, veh.diag.access }\ndistribution E { Lane }",
                &["RSDL-905"],
            ),
            (
                "distribution D { Lane, Panel, veh.diag.access, Backend }",
                &["RSDL-907"],
            ),
            // RSDL-901: `Panel`, in a `PLATFORM` distribution, requires an
            // interface `Lane` offers, in an `APPLICATION` one.
            (
                "distribution P [ tier = PLATFORM ] { Panel, veh.diag.access }\n\
                 distribution A [ tier = APPLICATION ] { Lane }",
                &["RSDL-901"],
            ),
            (
                "distribution P [ tier = APPLICATION ] { Panel, veh.diag.access }\n\
                 distribution A [ tier = PLATFORM ] { Lane }",
                &[],
            ),
            // A distribution without `tier` is exempt, on either side.
            (
                "distribution P [ tier = PLATFORM ] { Panel, veh.diag.access }\ndistribution A { Lane }",
                &[],
            ),
        ];
        for (distributions, expected) in cases {
            let text = format!(
                "package veh.topology\nimport veh.adas.LaneAssist\n{CLOSURE}{distributions}\n"
            );
            let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
            assert_eq!(codes(&system), *expected, "`{distributions}`");
            let facts = system
                .closure
                .and_then(|closure| closure.distribution_facts);
            assert_eq!(
                facts.is_some(),
                !distributions.is_empty(),
                "`{distributions}`"
            );
        }

        // With no system there is no closure, so no distribution rule runs
        // (rsdl §3.1).
        let system = check_topology(&[(
            "veh/topology/x.rsdl",
            "package veh.topology\ndistribution D { Nothing }\n",
        )]);
        assert_eq!(codes(&system), Vec::<&str>::new());
    }
}
