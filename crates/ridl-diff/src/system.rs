//! `ridl diff` at the system (rsdl reference §14): the changes to a lowered
//! system that are not contract changes.
//!
//! The contracts of a workspace are compared by the ridl categories, through
//! [`crate::diff_sets`]. What rsdl adds is structure — which components exist,
//! their lines, and where each instance runs — and a change to it changes the
//! derived links and never the contract. Such a change is listed under one of
//! two headings and carries no verdict: neither compatible nor breaking. Which
//! of them are breaking, and for whom, is the stability policy's.
//!
//! - **Placement changed** — a change in a deployment: a deployment added or
//!   removed, a machine added, removed or made `external`, an instance moved to
//!   another machine, or a `labels`, `deprecated` or backend key of the
//!   deployment, of a machine or of a placement line changed.
//! - **Composition changed** — a change to the closure or to a component's
//!   lines: a component added to or removed from the system, an `offers` or
//!   `requires` line added or removed, `instances` changed, a component made
//!   `external`, a distribution added or removed, or a `labels`, `deprecated`,
//!   `tier` or backend key of the
//!   system, of a component, of a distribution or of one of their lines
//!   changed.
//!
//! Both read the lowered `System` of each side and nothing else.

use ridl_ir::v2::{self, Component, Deployment, InterfaceRef, System};

/// The heading a system change is listed under (rsdl reference §14).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SystemHeading {
    /// A change in a deployment.
    PlacementChanged,
    /// A change to the closure or to a component's lines.
    CompositionChanged,
}

/// One change to the system, listed under its heading with no verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemChange {
    pub heading: SystemHeading,
    /// A slash-separated path: a component's or distribution's qualified
    /// name, optionally followed by a changed member or attribute; or a
    /// deployment's name, then a machine, an instance or an attribute.
    pub path: String,
    /// The rendered old value, absent when the change is an addition.
    pub before: Option<String>,
    /// The rendered new value, absent when the change is a removal.
    pub after: Option<String>,
}

/// The heading as the text report prints it: `placement changed`.
pub fn heading_text(heading: SystemHeading) -> &'static str {
    match heading {
        SystemHeading::PlacementChanged => "placement changed",
        SystemHeading::CompositionChanged => "composition changed",
    }
}

/// Compares two lowered systems: the composition changes first, in the new
/// closure's order and then the removed components, and then the placement
/// changes, deployment by deployment in the new system's order and then the
/// removed deployments. The doc fields (`doc`, the doc links, `see`,
/// `since`) are never read: a doc edit is neither a placement nor a
/// composition change, and a system change has no verdict (ADR-0026). The
/// sizing fields (`Deployment.sizing`, `Placement.sizing`) are never read: the
/// registry records them as not reported, reason sizing
/// (`ridl_core::attributes`).
pub fn diff_systems(old: &System, new: &System) -> Vec<SystemChange> {
    let mut changes = Vec::new();
    let system_path = format!("{}.{}", new.package, new.name);
    declaration_attrs(
        &mut changes,
        SystemHeading::CompositionChanged,
        &system_path,
        (&old.labels, old.deprecated.as_ref(), &old.attributes),
        (&new.labels, new.deprecated.as_ref(), &new.attributes),
    );
    for line in &new.members {
        if let Some(before) = old
            .members
            .iter()
            .find(|before| before.component == line.component)
        {
            line_attrs(
                &mut changes,
                SystemHeading::CompositionChanged,
                &format!("{system_path}/{}", line.component),
                &before.attributes,
                &line.attributes,
            );
        }
    }
    for component in &new.components {
        let name = component.qualified_name();
        match old
            .components
            .iter()
            .find(|before| before.qualified_name() == name)
        {
            Some(before) => compare_components(before, component, &mut changes),
            None => composition(&mut changes, name, None, Some("in the closure")),
        }
    }
    for component in &old.components {
        let name = component.qualified_name();
        if !new
            .components
            .iter()
            .any(|after| after.qualified_name() == name)
        {
            composition(&mut changes, name, Some("in the closure"), None);
        }
    }
    let empty_distribution = v2::Distribution::default();
    for distribution in &new.distributions {
        let name = distribution.qualified_name();
        let before = old
            .distributions
            .iter()
            .find(|before| before.qualified_name() == name);
        if before.is_none() {
            composition(&mut changes, name.clone(), None, Some("distribution"));
        }
        compare_distribution(
            before.unwrap_or(&empty_distribution),
            distribution,
            &name,
            &mut changes,
        );
    }
    for distribution in &old.distributions {
        let name = distribution.qualified_name();
        if !new
            .distributions
            .iter()
            .any(|after| after.qualified_name() == name)
        {
            composition(&mut changes, name.clone(), Some("distribution"), None);
            compare_distribution(distribution, &empty_distribution, &name, &mut changes);
        }
    }
    for deployment in &new.deployments {
        match old
            .deployments
            .iter()
            .find(|before| before.name == deployment.name)
        {
            Some(before) => compare_deployments(before, deployment, &mut changes),
            None => placement(
                &mut changes,
                deployment.name.clone(),
                None,
                Some("deployment"),
            ),
        }
    }
    for deployment in &old.deployments {
        if !new
            .deployments
            .iter()
            .any(|after| after.name == deployment.name)
        {
            placement(
                &mut changes,
                deployment.name.clone(),
                Some("deployment"),
                None,
            );
        }
    }
    changes
}

/// A component in both closures: its `offers` and `requires` lines, its
/// instances and its `external` flag.
fn compare_components(old: &Component, new: &Component, changes: &mut Vec<SystemChange>) {
    let name = new.qualified_name();
    let old_offers: Vec<&str> = old
        .offers
        .iter()
        .map(|offer| offer.service.as_str())
        .collect();
    let new_offers: Vec<&str> = new
        .offers
        .iter()
        .map(|offer| offer.service.as_str())
        .collect();
    lines(changes, &name, "offers", &old_offers, &new_offers);
    let old_requires: Vec<String> = old
        .requires
        .iter()
        .map(|require| interface_text(require.interface.as_ref()))
        .collect();
    let new_requires: Vec<String> = new
        .requires
        .iter()
        .map(|require| interface_text(require.interface.as_ref()))
        .collect();
    let old_requires: Vec<&str> = old_requires.iter().map(String::as_str).collect();
    let new_requires: Vec<&str> = new_requires.iter().map(String::as_str).collect();
    lines(changes, &name, "requires", &old_requires, &new_requires);
    if old.instances != new.instances {
        changes.push(SystemChange {
            heading: SystemHeading::CompositionChanged,
            path: format!("{name}/instances"),
            before: Some(instance_list(&old.instances)),
            after: Some(instance_list(&new.instances)),
        });
    }
    if old.external != new.external {
        changes.push(SystemChange {
            heading: SystemHeading::CompositionChanged,
            path: format!("{name}/external"),
            before: Some(old.external.to_string()),
            after: Some(new.external.to_string()),
        });
    }
    declaration_attrs(
        changes,
        SystemHeading::CompositionChanged,
        &name,
        (&old.labels, old.deprecated.as_ref(), &old.attributes),
        (&new.labels, new.deprecated.as_ref(), &new.attributes),
    );
    for offer in &new.offers {
        if let Some(before) = old
            .offers
            .iter()
            .find(|before| before.service == offer.service)
        {
            line_attrs(
                changes,
                SystemHeading::CompositionChanged,
                &format!("{name}/offers/{}", offer.service),
                &before.attributes,
                &offer.attributes,
            );
        }
    }
    for require in &new.requires {
        let interface = interface_text(require.interface.as_ref());
        if let Some(before) = old
            .requires
            .iter()
            .find(|before| interface_text(before.interface.as_ref()) == interface)
        {
            line_attrs(
                changes,
                SystemHeading::CompositionChanged,
                &format!("{name}/requires/{interface}"),
                &before.attributes,
                &require.attributes,
            );
        }
    }
}

/// The lines of one keyword that one side has and the other does not.
fn lines(
    changes: &mut Vec<SystemChange>,
    component: &str,
    keyword: &str,
    old: &[&str],
    new: &[&str],
) {
    for added in new.iter().filter(|line| !old.contains(line)) {
        composition(
            changes,
            format!("{component}/{keyword}/{added}"),
            None,
            Some(&format!("{keyword} {added}")),
        );
    }
    for removed in old.iter().filter(|line| !new.contains(line)) {
        composition(
            changes,
            format!("{component}/{keyword}/{removed}"),
            Some(&format!("{keyword} {removed}")),
            None,
        );
    }
}

/// A deployment in both systems: its machines, their `external` flags, and
/// the machine of every instance placed on both sides.
fn compare_deployments(old: &Deployment, new: &Deployment, changes: &mut Vec<SystemChange>) {
    declaration_attrs(
        changes,
        SystemHeading::PlacementChanged,
        &new.name,
        (&old.labels, old.deprecated.as_ref(), &old.attributes),
        (&new.labels, new.deprecated.as_ref(), &new.attributes),
    );
    for machine in &new.machines {
        let path = format!("{}/{}", new.name, machine.name);
        match old
            .machines
            .iter()
            .find(|before| before.name == machine.name)
        {
            Some(before) if before.external != machine.external => changes.push(SystemChange {
                heading: SystemHeading::PlacementChanged,
                path: format!("{path}/external"),
                before: Some(before.external.to_string()),
                after: Some(machine.external.to_string()),
            }),
            Some(_) => {}
            None => placement(changes, path, None, Some("machine")),
        }
        if let Some(before) = old
            .machines
            .iter()
            .find(|before| before.name == machine.name)
        {
            declaration_attrs(
                changes,
                SystemHeading::PlacementChanged,
                &format!("{}/{}", new.name, machine.name),
                (
                    &before.labels,
                    before.deprecated.as_ref(),
                    &before.attributes,
                ),
                (
                    &machine.labels,
                    machine.deprecated.as_ref(),
                    &machine.attributes,
                ),
            );
        }
    }
    for machine in &old.machines {
        if !new.machines.iter().any(|after| after.name == machine.name) {
            placement(
                changes,
                format!("{}/{}", new.name, machine.name),
                Some("machine"),
                None,
            );
        }
    }
    for placed in &new.placements {
        let paired = old.placements.iter().find(|before| {
            before.component == placed.component && before.instance == placed.instance
        });
        let Some(before) = paired else {
            continue;
        };
        if before.machine != placed.machine {
            changes.push(SystemChange {
                heading: SystemHeading::PlacementChanged,
                path: format!("{}/{}.{}", new.name, placed.component, placed.instance),
                before: Some(before.machine.clone()),
                after: Some(placed.machine.clone()),
            });
        }
        line_attrs(
            changes,
            SystemHeading::PlacementChanged,
            &format!("{}/{}.{}", new.name, placed.component, placed.instance),
            &before.attributes,
            &placed.attributes,
        );
    }
}

fn compare_distribution(
    before: &v2::Distribution,
    after: &v2::Distribution,
    name: &str,
    changes: &mut Vec<SystemChange>,
) {
    value_change(
        changes,
        SystemHeading::CompositionChanged,
        format!("{name}/tier"),
        tier_text(before.tier),
        tier_text(after.tier),
    );
    declaration_attrs(
        changes,
        SystemHeading::CompositionChanged,
        name,
        (
            &before.labels,
            before.deprecated.as_ref(),
            &before.attributes,
        ),
        (&after.labels, after.deprecated.as_ref(), &after.attributes),
    );
    // Unpaired distribution members and their attributes are not reported
    // yet (driftsys/ridl#974).
    for line in &after.members {
        if let Some(before) = before
            .members
            .iter()
            .find(|earlier| earlier.component == line.component)
        {
            line_attrs(
                changes,
                SystemHeading::CompositionChanged,
                &format!("{name}/{}", line.component),
                &before.attributes,
                &line.attributes,
            );
        }
    }
}

fn composition(
    changes: &mut Vec<SystemChange>,
    path: String,
    before: Option<&str>,
    after: Option<&str>,
) {
    changes.push(SystemChange {
        heading: SystemHeading::CompositionChanged,
        path,
        before: before.map(str::to_string),
        after: after.map(str::to_string),
    });
}

fn placement(
    changes: &mut Vec<SystemChange>,
    path: String,
    before: Option<&str>,
    after: Option<&str>,
) {
    changes.push(SystemChange {
        heading: SystemHeading::PlacementChanged,
        path,
        before: before.map(str::to_string),
        after: after.map(str::to_string),
    });
}

/// `(A, B)` as `labels` is written; `()` when empty.
fn label_list(labels: &[String]) -> String {
    format!("({})", labels.join(", "))
}

/// `[ ns.key = value, ns.flag ]` as a backend key block is written; `[]`
/// when empty.
fn attribute_list(attributes: &[v2::Attribute]) -> String {
    if attributes.is_empty() {
        return "[]".to_string();
    }
    let items: Vec<String> = attributes
        .iter()
        .map(|attribute| match &attribute.value {
            Some(value) => format!(
                "{}.{} = {}",
                attribute.namespace,
                attribute.key,
                value_text(value)
            ),
            None => format!("{}.{}", attribute.namespace, attribute.key),
        })
        .collect();
    format!("[ {} ]", items.join(", "))
}

/// An attribute value as written: a scalar as its text, a list as `(a, b)`.
fn value_text(value: &v2::AttributeValue) -> String {
    match &value.kind {
        Some(v2::attribute_value::Kind::Scalar(text)) => text.clone(),
        Some(v2::attribute_value::Kind::List(list)) => {
            let items: Vec<String> = list.items.iter().map(value_text).collect();
            format!("({})", items.join(", "))
        }
        None => String::new(),
    }
}

/// `PLATFORM` or `APPLICATION`, or `None` when the distribution has no tier.
/// A value this build does not know renders as `tier(<n>)`, so that a
/// snapshot written by a later toolchain does not read as "no tier".
fn tier_text(tier: i32) -> Option<String> {
    match v2::Tier::try_from(tier) {
        Ok(v2::Tier::Platform) => Some("PLATFORM".to_string()),
        Ok(v2::Tier::Application) => Some("APPLICATION".to_string()),
        Ok(v2::Tier::Unspecified) => None,
        Err(_) => Some(format!("tier({tier})")),
    }
}

/// One before-and-after line when the two values differ.
fn value_change(
    changes: &mut Vec<SystemChange>,
    heading: SystemHeading,
    path: String,
    before: Option<String>,
    after: Option<String>,
) {
    if before != after {
        changes.push(SystemChange {
            heading,
            path,
            before,
            after,
        });
    }
}

/// The `labels`, `deprecated` and backend keys of a declaration, under
/// `heading` at `path`.
fn declaration_attrs(
    changes: &mut Vec<SystemChange>,
    heading: SystemHeading,
    path: &str,
    old: (&[String], Option<&String>, &[v2::Attribute]),
    new: (&[String], Option<&String>, &[v2::Attribute]),
) {
    value_change(
        changes,
        heading,
        format!("{path}/labels"),
        Some(label_list(old.0)),
        Some(label_list(new.0)),
    );
    value_change(
        changes,
        heading,
        format!("{path}/deprecated"),
        old.1.map(|reason| format!("{reason:?}")),
        new.1.map(|reason| format!("{reason:?}")),
    );
    line_attrs(changes, heading, path, old.2, new.2);
}

/// The backend keys of a line, under `heading` at `path`.
fn line_attrs(
    changes: &mut Vec<SystemChange>,
    heading: SystemHeading,
    path: &str,
    old: &[v2::Attribute],
    new: &[v2::Attribute],
) {
    value_change(
        changes,
        heading,
        format!("{path}/attributes"),
        Some(attribute_list(old)),
        Some(attribute_list(new)),
    );
}

/// `catalog.Name` for a declared interface, the service's dotted name for an
/// inline shape.
fn interface_text(interface: Option<&InterfaceRef>) -> String {
    match interface {
        Some(interface) if interface.inline => interface.name.clone(),
        Some(interface) => format!("{}.{}", interface.catalog, interface.name),
        None => String::new(),
    }
}

/// `(primary, backup)`, as `instances` is written.
fn instance_list(instances: &[String]) -> String {
    format!("({})", instances.join(", "))
}

#[cfg(test)]
mod tests {
    use ridl_ir::v2;

    use super::*;

    fn component(name: &str, instances: &[&str], offers: &[&str], requires: &[&str]) -> Component {
        Component {
            name: name.to_string(),
            package: "veh.topology".to_string(),
            implicit: false,
            external: false,
            instances: instances
                .iter()
                .map(|instance| instance.to_string())
                .collect(),
            offers: offers
                .iter()
                .map(|service| v2::Offer {
                    service: service.to_string(),
                    attributes: Vec::new(),
                    doc: String::new(),
                    links: Vec::new(),
                    see: Vec::new(),
                    since: Vec::new(),
                })
                .collect(),
            requires: requires
                .iter()
                .map(|interface| v2::Require {
                    interface: Some(v2::InterfaceRef {
                        catalog: "veh.adas".to_string(),
                        name: interface.to_string(),
                        inline: false,
                    }),
                    service: String::new(),
                    producer: String::new(),
                    attributes: Vec::new(),
                    doc: String::new(),
                    links: Vec::new(),
                    see: Vec::new(),
                    since: Vec::new(),
                })
                .collect(),
            labels: Vec::new(),
            attributes: Vec::new(),
            doc: String::new(),
            links: Vec::new(),
            see: Vec::new(),
            since: Vec::new(),
            deprecated: None,
        }
    }

    fn machine(name: &str, external: bool) -> v2::Machine {
        v2::Machine {
            name: name.to_string(),
            external,
            labels: Vec::new(),
            attributes: Vec::new(),
            doc: String::new(),
            links: Vec::new(),
            see: Vec::new(),
            since: Vec::new(),
            deprecated: None,
        }
    }

    fn placed(component: &str, instance: &str, machine: &str) -> v2::Placement {
        v2::Placement {
            component: format!("veh.topology.{component}"),
            instance: instance.to_string(),
            machine: machine.to_string(),
            attributes: Vec::new(),
            sizing: None,
        }
    }

    /// Appendix A reduced to `Cruise` and `Panel` in `Production`.
    fn base() -> System {
        System {
            name: "Vehicle".to_string(),
            package: "veh.topology".to_string(),
            components: vec![
                component(
                    "Cruise",
                    &["primary", "backup"],
                    &["veh.adas.cruise"],
                    &["LaneAssist"],
                ),
                component("Panel", &["Unit"], &[], &["CruiseControl"]),
            ],
            deployments: vec![Deployment {
                name: "Production".to_string(),
                package: "veh.topology".to_string(),
                machines: vec![machine("AdasHpc", false), machine("Cockpit", false)],
                placements: vec![
                    placed("Cruise", "primary", "AdasHpc"),
                    placed("Cruise", "backup", "Cockpit"),
                    placed("Panel", "Unit", "Cockpit"),
                ],
                ..Deployment::default()
            }],
            ..System::default()
        }
    }

    /// Each change as one line, `<heading>: <path>: <before> -> <after>`.
    fn lines(changes: &[SystemChange]) -> Vec<String> {
        changes
            .iter()
            .map(|change| {
                let mut line = format!("{}: {}", heading_text(change.heading), change.path);
                crate::push_values(&mut line, change.before.as_ref(), change.after.as_ref());
                line
            })
            .collect()
    }

    fn backend(namespace: &str, key: &str, value: Option<&str>) -> v2::Attribute {
        v2::Attribute {
            namespace: namespace.to_string(),
            key: key.to_string(),
            value: value.map(|text| v2::AttributeValue {
                kind: Some(v2::attribute_value::Kind::Scalar(text.to_string())),
            }),
        }
    }

    /// `base()` plus one distribution and the system's own member lines, so
    /// that every rsdl site is present.
    fn with_distribution() -> System {
        let mut system = base();
        system.members = vec![
            v2::MemberLine {
                component: "veh.topology.Cruise".to_string(),
                ..Default::default()
            },
            v2::MemberLine {
                component: "veh.topology.Panel".to_string(),
                ..Default::default()
            },
        ];
        system.distributions.push(v2::Distribution {
            name: "Adas".to_string(),
            package: "veh.topology".to_string(),
            members: vec![v2::MemberLine {
                component: "veh.topology.Cruise".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        });
        system
    }

    #[test]
    fn bare_distributions_are_reported_in_both_directions() {
        let present = with_distribution();
        let mut absent = present.clone();
        absent.distributions.clear();
        let change = SystemChange {
            heading: SystemHeading::CompositionChanged,
            path: "veh.topology.Adas".to_string(),
            before: None,
            after: Some("distribution".to_string()),
        };
        assert_eq!(diff_systems(&absent, &present), [change.clone()]);
        assert_eq!(
            diff_systems(&present, &absent),
            [SystemChange {
                before: change.after,
                after: change.before,
                ..change
            }]
        );
    }

    #[test]
    fn added_and_removed_distributions_include_their_attributes() {
        let mut old = with_distribution();
        // An unchanged distribution with the same short name must not hide
        // the changed distribution from a different package.
        old.distributions.push(v2::Distribution {
            name: "Adas".to_string(),
            package: "veh.other".to_string(),
            ..Default::default()
        });
        let mut new = old.clone();
        let distribution = &mut new.distributions[0];
        distribution.tier = v2::Tier::Platform as i32;
        distribution.labels = vec!["QM".to_string()];
        distribution.deprecated = Some("use Next".to_string());
        distribution.attributes = vec![backend("sign", "key", Some("release"))];
        let mut absent = old.clone();
        absent
            .distributions
            .retain(|distribution| distribution.package != "veh.topology");
        let added = diff_systems(&absent, &new);
        let expected = [
            ("veh.topology.Adas", None, Some("distribution")),
            ("veh.topology.Adas/tier", None, Some("PLATFORM")),
            ("veh.topology.Adas/labels", Some("()"), Some("(QM)")),
            ("veh.topology.Adas/deprecated", None, Some("\"use Next\"")),
            (
                "veh.topology.Adas/attributes",
                Some("[]"),
                Some("[ sign.key = release ]"),
            ),
        ];
        assert_eq!(added.len(), expected.len(), "{added:?}");
        for (change, (path, before, after)) in added.iter().zip(expected) {
            assert_eq!(change.heading, SystemHeading::CompositionChanged);
            assert_eq!(change.path, path);
            assert_eq!(change.before.as_deref(), before);
            assert_eq!(change.after.as_deref(), after);
        }
        assert_eq!(diff_systems(&old, &new), added[1..]);
        let removed = diff_systems(&new, &absent);
        assert_eq!(removed.len(), added.len());
        for (removed, added) in removed.iter().zip(&added) {
            assert_eq!(removed.heading, added.heading);
            assert_eq!(removed.path, added.path);
            assert_eq!(removed.before, added.after);
            assert_eq!(removed.after, added.before);
        }
    }

    /// Every `Compared` row of the registry with an rsdl site is reported by
    /// `diff_systems`, and every `NotReported` row is not. The mutation per
    /// row is written here, so a row added to the registry without diff
    /// support fails this test on the `panic!` arm.
    #[test]
    fn the_registry_rows_are_reported_as_recorded() {
        use ridl_core::attributes::{ATTRIBUTES, AttrName, Diff, Site};
        for row in ATTRIBUTES {
            if !row
                .sites
                .iter()
                .any(|site| !matches!(site, Site::Command | Site::Query))
            {
                continue;
            }
            let old = with_distribution();
            let mut new = with_distribution();
            let key = match row.name {
                AttrName::Key(key) => key,
                AttrName::Namespaced => "ns.key",
            };
            match key {
                "instances" => new.components[0].instances = vec!["Unit".to_string()],
                "external" => new.components[0].external = true,
                "tier" => new.distributions[0].tier = v2::Tier::Application as i32,
                "labels" => new.distributions[0].labels = vec!["QM".to_string()],
                "deprecated" => new.deployments[0].deprecated = Some("use v2".to_string()),
                "depth" => {
                    new.deployments[0].sizing = Some(v2::Sizing {
                        depth: Some(4),
                        slots: None,
                        budget: None,
                    })
                }
                "slots" => {
                    new.deployments[0].placements[0].sizing = Some(v2::Sizing {
                        depth: None,
                        slots: Some(4),
                        budget: None,
                    })
                }
                "budget" => {
                    new.deployments[0].sizing = Some(v2::Sizing {
                        depth: None,
                        slots: None,
                        budget: Some(4096),
                    })
                }
                "ns.key" => new.deployments[0].machines[0].attributes.push(backend(
                    "linux",
                    "cpuset",
                    Some("(2)"),
                )),
                other => panic!("registry row `{other}` has no mutation in this test"),
            }
            let changes = diff_systems(&old, &new);
            match row.diff {
                Diff::Compared(_) => assert!(!changes.is_empty(), "`{key}` must be reported"),
                Diff::NotReported(reason) => assert!(
                    changes.is_empty(),
                    "`{key}` is recorded as not reported ({reason:?}) but was: {changes:?}"
                ),
            }
        }
    }

    /// rsdl §14: `labels`, `deprecated`, `tier` and a backend key are listed
    /// under the heading of the declaration they are on, as a before-and-after
    /// line. Every site the registry names is exercised here: the system and
    /// its member lines, a component with its `offers` and `requires` lines, a
    /// distribution with its member lines, a deployment, a machine and a
    /// placement line.
    #[test]
    fn attribute_changes_are_listed_under_the_heading_of_their_site() {
        let old = with_distribution();
        let mut new = with_distribution();
        new.labels = vec!["QM".to_string()];
        new.components[0].deprecated = Some("use Cruise2".to_string());
        new.distributions[0].tier = v2::Tier::Platform as i32;
        new.members[0]
            .attributes
            .push(backend("deb", "section", Some("net")));
        new.components[0].requires[0]
            .attributes
            .push(backend("someip", "clientId", Some("17")));
        new.distributions[0].members[0]
            .attributes
            .push(backend("sign", "key", None));
        new.components[0].offers[0]
            .attributes
            .push(backend("someip", "serviceId", Some("4660")));
        new.deployments[0]
            .attributes
            .push(backend("rust", "crate", Some("\"s\"")));
        new.deployments[0].machines[0].labels = vec!["ASIL_B".to_string()];
        new.deployments[0].placements[0]
            .attributes
            .push(backend("linux", "cpuset", Some("(2)")));
        assert_eq!(
            lines(&diff_systems(&old, &new)),
            [
                "composition changed: veh.topology.Vehicle/labels: () -> (QM)",
                "composition changed: veh.topology.Vehicle/veh.topology.Cruise/attributes: \
                 [] -> [ deb.section = net ]",
                "composition changed: veh.topology.Cruise/deprecated: (absent) -> \"use Cruise2\"",
                "composition changed: veh.topology.Cruise/offers/veh.adas.cruise/attributes: \
                 [] -> [ someip.serviceId = 4660 ]",
                "composition changed: veh.topology.Cruise/requires/veh.adas.LaneAssist/attributes: \
                 [] -> [ someip.clientId = 17 ]",
                "composition changed: veh.topology.Adas/tier: (absent) -> PLATFORM",
                "composition changed: veh.topology.Adas/veh.topology.Cruise/attributes: \
                 [] -> [ sign.key ]",
                "placement changed: Production/attributes: [] -> [ rust.crate = \"s\" ]",
                "placement changed: Production/AdasHpc/labels: () -> (ASIL_B)",
                "placement changed: Production/veh.topology.Cruise.primary/attributes: \
                 [] -> [ linux.cpuset = (2) ]",
            ]
        );
    }

    /// A value removed is reported, as the mirror of one added.
    #[test]
    fn an_attribute_removed_is_reported() {
        let mut old = with_distribution();
        old.components[0].deprecated = Some("use Cruise2".to_string());
        old.distributions[0].tier = v2::Tier::Platform as i32;
        let new = with_distribution();
        assert_eq!(
            lines(&diff_systems(&old, &new)),
            [
                "composition changed: veh.topology.Cruise/deprecated: \"use Cruise2\" -> (removed)",
                "composition changed: veh.topology.Adas/tier: PLATFORM -> (removed)",
            ]
        );
    }

    #[test]
    fn an_unchanged_system_has_no_system_change() {
        assert!(diff_systems(&base(), &base()).is_empty());
    }

    /// The region map is not compared, so a change to a region's catalog hash
    /// lists no system change.
    #[test]
    fn a_region_hash_change_is_not_a_system_change() {
        let region = |hash: u8| v2::Region {
            catalog: "veh.adas".to_string(),
            interfaces: Vec::new(),
            hash: vec![hash; 32],
        };
        let old = System {
            regions: vec![region(1)],
            ..base()
        };
        let new = System {
            regions: vec![region(2)],
            ..base()
        };
        assert!(diff_systems(&old, &new).is_empty());
    }

    /// A doc edit is neither a placement nor a composition change, so the
    /// doc fields of every system message are ignored (ADR-0026).
    #[test]
    fn system_doc_changes_are_not_reported() {
        let see = v2::DocLink {
            text: "Other".to_string(),
            target: "veh.topology.Other".to_string(),
            ..Default::default()
        };
        macro_rules! document {
            ($carrier:expr) => {{
                let carrier = &mut $carrier;
                carrier.doc = "documented".to_string();
                carrier.links.push(see.clone());
                carrier.see.push(see.clone());
                carrier.since.push("1.2".to_string());
            }};
        }
        let mut old = base();
        old.members.push(v2::MemberLine {
            component: "veh.topology.Cruise".to_string(),
            ..Default::default()
        });
        old.distributions.push(v2::Distribution {
            name: "Adas".to_string(),
            package: "veh.topology".to_string(),
            members: old.members.clone(),
            ..Default::default()
        });
        let mut new = old.clone();
        document!(new);
        document!(new.members[0]);
        document!(new.components[0]);
        document!(new.components[0].offers[0]);
        document!(new.components[0].requires[0]);
        document!(new.distributions[0]);
        document!(new.distributions[0].members[0]);
        let deployment = &mut new.deployments[0];
        deployment.doc = "documented".to_string();
        deployment.doc_links.push(see.clone());
        deployment.see.push(see.clone());
        deployment.since.push("1.2".to_string());
        document!(new.deployments[0].machines[0]);
        assert_ne!(old, new);
        assert!(diff_systems(&old, &new).is_empty());
    }

    /// A changed sizing value, on the deployment or on a placement, is
    /// neither a placement nor a composition change (rsdl §14).
    #[test]
    fn system_sizing_changes_are_not_reported() {
        let sized = |depth: u32, slots: u32, budget: u64| v2::Sizing {
            depth: Some(depth),
            slots: Some(slots),
            budget: Some(budget),
        };
        let mut old = base();
        old.deployments[0].sizing = Some(sized(3, 16, 4096));
        old.deployments[0].placements[0].sizing = Some(sized(3, 16, 4096));
        let mut new = old.clone();
        new.deployments[0].sizing = Some(sized(300, 64, 65536));
        new.deployments[0].placements[0].sizing = Some(sized(300, 64, 65536));
        assert_ne!(old, new);
        assert!(diff_systems(&old, &new).is_empty());

        // Declaring a value where none was, and removing one, are not
        // reported either.
        assert!(diff_systems(&base(), &new).is_empty());
        assert!(diff_systems(&new, &base()).is_empty());
    }

    /// A system change is rendered under its heading, after the contract
    /// changes and with no verdict, and leaves the report verdict alone; a
    /// system on one side only is not compared (rsdl §14).
    #[test]
    fn system_changes_render_under_their_headings_with_no_verdict() {
        let old = base();
        let mut new = base();
        new.deployments[0].placements[1].machine = "AdasHpc".to_string();
        new.components[1].external = true;

        let report = crate::diff_workspaces(&[], Some(&old), &[], Some(&new), &[]);
        assert_eq!(report.verdict, crate::Verdict::Identical);
        assert_eq!(
            crate::render_text(&report),
            "identical\n\
             placement changed\n\
             \x20 Production/veh.topology.Cruise.backup: Cockpit -> AdasHpc\n\
             composition changed\n\
             \x20 veh.topology.Panel/external: false -> true\n"
        );
        let json: serde_json::Value =
            serde_json::from_str(&crate::render_json(&report)).expect("the report is JSON");
        assert_eq!(
            json["placement_changed"],
            serde_json::json!([{
                "path": "Production/veh.topology.Cruise.backup",
                "before": "Cockpit",
                "after": "AdasHpc"
            }])
        );
        assert_eq!(json["composition_changed"][0]["after"], "true");

        let one_side = crate::diff_workspaces(&[], None, &[], Some(&new), &[]);
        assert!(one_side.system.is_empty());
        let json: serde_json::Value =
            serde_json::from_str(&crate::render_json(&one_side)).expect("the report is JSON");
        assert_eq!(
            json,
            serde_json::json!({ "verdict": "identical", "changes": [] })
        );
    }

    /// rsdl §14: an instance moved to another machine, a machine added,
    /// removed or made `external`, and a deployment added or removed are
    /// placement changes.
    #[test]
    fn a_deployment_change_is_listed_under_placement_changed() {
        let old = base();
        let mut new = base();
        new.deployments[0].placements[1].machine = "AdasHpc".to_string();
        new.deployments[0].machines[1].external = true;
        new.deployments[0].machines.push(machine("Cloud", true));
        new.deployments.push(Deployment {
            name: "Bench".to_string(),
            ..Deployment::default()
        });
        assert_eq!(
            lines(&diff_systems(&old, &new)),
            [
                "placement changed: Production/Cockpit/external: false -> true",
                "placement changed: Production/Cloud: (absent) -> machine",
                "placement changed: Production/veh.topology.Cruise.backup: Cockpit -> AdasHpc",
                "placement changed: Bench: (absent) -> deployment",
            ]
        );
        assert_eq!(
            lines(&diff_systems(&new, &old)),
            [
                "placement changed: Production/Cockpit/external: true -> false",
                "placement changed: Production/Cloud: machine -> (removed)",
                "placement changed: Production/veh.topology.Cruise.backup: AdasHpc -> Cockpit",
                "placement changed: Bench: deployment -> (removed)",
            ]
        );
    }

    /// rsdl §14: a component added to or removed from the system, an `offers`
    /// or `requires` line added or removed, `instances` changed, and a
    /// component made `external` are composition changes.
    #[test]
    fn a_closure_or_line_change_is_listed_under_composition_changed() {
        let old = base();
        let mut new = base();
        new.components[0].instances = vec!["Unit".to_string()];
        new.components[0].requires.clear();
        new.components[1].external = true;
        new.components[1].offers.push(v2::Offer {
            service: "veh.hmi.panel".to_string(),
            attributes: Vec::new(),
            doc: String::new(),
            links: Vec::new(),
            see: Vec::new(),
            since: Vec::new(),
        });
        new.components
            .push(component("Lane", &["Unit"], &["veh.adas.lane"], &[]));
        assert_eq!(
            lines(&diff_systems(&old, &new)),
            [
                "composition changed: veh.topology.Cruise/requires/veh.adas.LaneAssist: \
                 requires veh.adas.LaneAssist -> (removed)",
                "composition changed: veh.topology.Cruise/instances: (primary, backup) -> (Unit)",
                "composition changed: veh.topology.Panel/offers/veh.hmi.panel: \
                 (absent) -> offers veh.hmi.panel",
                "composition changed: veh.topology.Panel/external: false -> true",
                "composition changed: veh.topology.Lane: (absent) -> in the closure",
            ]
        );
        assert_eq!(
            lines(&diff_systems(&new, &old)).last().map(String::as_str),
            Some("composition changed: veh.topology.Lane: in the closure -> (removed)")
        );
    }
}
