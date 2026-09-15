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
mod collect;

use std::collections::HashMap;

use ridl_core::db::InputFile;
use ridl_core::diag::{DiagCode, Diagnostic, FileId, Severity, SourceMap, Span};
use ridl_core::package::{Package, Workspace};
use rowan::TextRange;

/// The name of the unit instance: the one instance of a component that
/// declares no `instances` (rsdl §7). It is never written in source; a written
/// `Unit` is kept in the model so that its diagnostic (RSDL-307) can report it.
pub const UNIT_INSTANCE: &str = "Unit";

/// Checks every `.rsdl` file of `ws` and returns the collected model with its
/// diagnostics (rsdl reference v0.2).
///
/// `std` is the embedded `ridl.std` package, threaded in for signature parity
/// with `service_catalog`; it declares no rsdl, so it contributes nothing.
#[salsa::tracked(returns(clone))]
pub fn check_system(db: &dyn salsa::Database, ws: Workspace, std: Package) -> CheckedSystem {
    let _ = std;
    let mut reporter = Reporter::new(db, ws);
    let mut system = collect::collect(db, ws, &mut reporter);
    system.diagnostics = reporter.diagnostics;
    system
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
}

/// One body line: a member line of a `system`, `distribution` or `machine`,
/// or an `offers`/`requires` line of a `component` (rsdl §4). A line takes
/// backend keys only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRef {
    pub reference: Reference,
    pub backend_keys: Vec<BackendKey>,
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
        let file = self
            .file_ids
            .get(&site.file)
            .copied()
            .unwrap_or(FileId::DETACHED);
        self.diagnostics.push(Diagnostic {
            code,
            severity: Severity::Error,
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
    fn package(db: &RidlDatabase, name: &str, files: &[(&str, &str)]) -> Package {
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
        )
    }

    /// Checks a workspace made of `packages`, each `(name, files)`.
    fn check(packages: &[(&str, &[(&str, &str)])]) -> CheckedSystem {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let packages = packages
            .iter()
            .map(|(name, files)| package(&db, name, files))
            .collect();
        let ws = Workspace::new(&db, packages, BTreeMap::new());
        check_system(&db, ws, std)
    }

    fn codes(system: &CheckedSystem) -> Vec<&str> {
        system.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    /// rsdl reference Appendix A, `system.rsdl`, verbatim.
    const SYSTEM: &str = r#"package veh.topology

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
    const PRODUCTION: &str = r#"package veh.topology

deployment Production for Vehicle {
  machine AdasHpc [ labels = (ASIL_B) ] { Cruise.primary, Lane, veh.diag.access }
  machine Cockpit { Cruise.backup, Panel [ linux.cpuset = (2, 3) ] }
  machine Cloud   [ external ] { Backend }
}
"#;

    fn texts(lines: &[MemberRef]) -> Vec<String> {
        lines.iter().map(|line| line.reference.text()).collect()
    }

    #[test]
    fn appendix_a_collects_into_the_model_with_no_diagnostic() {
        let system = check(&[(
            "veh.topology",
            &[
                ("veh/topology/system.rsdl", SYSTEM),
                ("veh/topology/production.rsdl", PRODUCTION),
            ],
        )]);
        assert_eq!(codes(&system), Vec::<&str>::new());

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
            // FORM-107: a key its row does not name, and any rsdl key on a line.
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
            assert_eq!(codes(&system), *expected, "`{decl}`");
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
}
