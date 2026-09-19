//! `ridl diff` at the system (rsdl reference §14): the changes to a lowered
//! system that are not contract changes.
//!
//! The contracts of a workspace are compared by the ridl categories, through
//! [`crate::diff_sets`]. What rsdl adds is structure — which components exist,
//! their lines, and where each instance runs — and a change to it changes the
//! derived links and never the contract. Such a change is listed under one of
//! two headings and carries no verdict: neither compatible nor breaking. Which
//! of them are breaking, and for whom, is the stability policy's (roadmap
//! E4.5a).
//!
//! - **Placement changed** — a change in a deployment: a deployment added or
//!   removed, a machine added, removed or made `external`, an instance moved to
//!   another machine.
//! - **Composition changed** — a change to the closure or to a component's
//!   lines: a component added to or removed from the system, an `offers` or
//!   `requires` line added or removed, `instances` changed, a component made
//!   `external`.
//!
//! Both read the lowered `System` of each side and nothing else.

use ridl_ir::v2::{Component, Deployment, InterfaceRef, System};

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
    /// A slash-separated path: a component's qualified name, then `offers`,
    /// `requires`, `instances` or `external`; or a deployment's name, then a
    /// machine or an instance.
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
/// removed deployments.
pub fn diff_systems(old: &System, new: &System) -> Vec<SystemChange> {
    let mut changes = Vec::new();
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
        let moved_from = old.placements.iter().find(|before| {
            before.component == placed.component
                && before.instance == placed.instance
                && before.machine != placed.machine
        });
        if let Some(before) = moved_from {
            changes.push(SystemChange {
                heading: SystemHeading::PlacementChanged,
                path: format!("{}/{}.{}", new.name, placed.component, placed.instance),
                before: Some(before.machine.clone()),
                after: Some(placed.machine.clone()),
            });
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
                })
                .collect(),
            labels: Vec::new(),
            attributes: Vec::new(),
        }
    }

    fn machine(name: &str, external: bool) -> v2::Machine {
        v2::Machine {
            name: name.to_string(),
            external,
            labels: Vec::new(),
            attributes: Vec::new(),
        }
    }

    fn placed(component: &str, instance: &str, machine: &str) -> v2::Placement {
        v2::Placement {
            component: format!("veh.topology.{component}"),
            instance: instance.to_string(),
            machine: machine.to_string(),
            attributes: Vec::new(),
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

    #[test]
    fn an_unchanged_system_has_no_system_change() {
        assert!(diff_systems(&base(), &base()).is_empty());
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

        let report = crate::diff_workspaces(&[], Some(&old), &[], Some(&new));
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

        let one_side = crate::diff_workspaces(&[], None, &[], Some(&new));
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
