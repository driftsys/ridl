//! Deployments, machines and placement (rsdl reference §3.4, §3.5, §9).
//!
//! Every deployment is checked on its own. The name rules (RSDL-708 across
//! the workspace, RSDL-705 within a deployment) and the `for` rule (RSDL-704)
//! run on every deployment. The placement rules quantify over the closure
//! (§3.1), so they run on a deployment `for` the closure's system: RSDL-702 for
//! a line outside the closure, RSDL-706 for an instance placed twice, RSDL-707
//! for an implemented component on an external machine, and RSDL-701 for an
//! unplaced instance. A deployment `for` a second `system` is not placed: that
//! system is RSDL-601, which blocks every deployment.
//!
//! rsdl §13 blocks lowering for one deployment on an RSDL-7xx error and for
//! every deployment on any other. [`DeploymentPlacement::has_errors`] records
//! the first kind per deployment, and also a deployment whose closure was
//! never placed even though no RSDL-7xx error was raised for it;
//! `CheckedSystem::closure_has_errors` records the second.

use std::collections::HashMap;

use ridl_core::diag::DiagCode;

use super::closure::{Closure, Lookup, MemberTarget, member_target};
use super::{CheckedSystem, DeploymentDecl, ReferenceForm, Reporter, Site};

/// The placement of the closure in one deployment (rsdl §9).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeploymentPlacement {
    /// Whether an RSDL-7xx error was raised for this deployment (rsdl §13): its
    /// lowering is blocked, and no other deployment's is. Also set when the
    /// closure was never placed in this deployment, even though no RSDL-7xx
    /// error was raised for it.
    pub has_errors: bool,
    /// Every placement the machine lines make, in line order; an instance
    /// placed twice keeps its first placement (RSDL-706). Empty when the
    /// deployment is not `for` the closure's system.
    pub placements: Vec<Placement>,
}

/// One instance on one machine (rsdl §9, §13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    /// The component, as an index into `Closure::components`.
    pub component: usize,
    /// The instance name: a declared instance, or `Unit`.
    pub instance: String,
    /// The machine, as an index into `DeploymentDecl::machines`; its name,
    /// `external` flag and `labels` are there.
    pub machine: usize,
    /// The placement line, as an index into that machine's
    /// `MachineDecl::members`; its backend keys apply to this placement.
    pub line: usize,
}

/// Checks every deployment and places the closure (rsdl §3.4, §3.5, §9).
/// Returns one entry per `CheckedSystem::deployments`, in order.
pub(super) fn place(
    lookup: &Lookup,
    system: &CheckedSystem,
    closure: Option<&Closure>,
    reporter: &mut Reporter,
) -> Vec<DeploymentPlacement> {
    let mut placements = Vec::new();
    for (index, deployment) in system.deployments.iter().enumerate() {
        let mut check = DeploymentCheck {
            reporter: &mut *reporter,
            has_errors: false,
        };
        if let Some(first) = system.deployments[..index]
            .iter()
            .find(|earlier| earlier.name.name == deployment.name.name)
        {
            check.error(
                DiagCode::RSDL_708,
                deployment.name.site,
                format!(
                    "a deployment named `{}` is already declared in the workspace — a deployment \
                     name is unique (rsdl reference §3.4)",
                    first.name.name
                ),
            );
        }
        for (position, machine) in deployment.machines.iter().enumerate() {
            if deployment.machines[..position]
                .iter()
                .any(|earlier| earlier.name.name == machine.name.name)
            {
                check.error(
                    DiagCode::RSDL_705,
                    machine.name.site,
                    format!(
                        "`{}` is declared twice in `{}` — a machine name is unique in its \
                         deployment (rsdl reference §3.5)",
                        machine.name.name, deployment.name.name
                    ),
                );
            }
        }
        let placed = match (for_system(lookup, deployment, &mut check), closure) {
            (Some(0), Some(closure)) => place_one(lookup, system, deployment, closure, &mut check),
            // The closure was not placed in this deployment, so `placements`
            // does not cover it and the lowering must not read it (rsdl §13):
            // the deployment is blocked, exactly as a placement error blocks
            // it. Each case that reaches here is already reported — RSDL-704
            // for a `for` that names no system, RSDL-601 for a second system,
            // FORM-101 for a `for` the parser recovered without a reference —
            // but the last is a parse error, which the rsdl reporter never
            // sees, so the flag is set here rather than at each report.
            _ => {
                check.has_errors = true;
                Vec::new()
            }
        };
        placements.push(DeploymentPlacement {
            has_errors: check.has_errors,
            placements: placed,
        });
    }
    placements
}

/// Reports an RSDL-7xx error and marks the deployment blocked (rsdl §13).
struct DeploymentCheck<'r> {
    reporter: &'r mut Reporter,
    has_errors: bool,
}

impl DeploymentCheck<'_> {
    fn error(&mut self, code: DiagCode, site: Site, message: String) {
        self.has_errors = true;
        self.reporter.error(code, site, message);
    }
}

/// The system a deployment is `for`, as an index into
/// `CheckedSystem::systems`. A reference that resolves to no declared system
/// is RSDL-704; a `for` the parser recovered without a reference is `None`
/// with its parse error already reported.
fn for_system(
    lookup: &Lookup,
    deployment: &DeploymentDecl,
    check: &mut DeploymentCheck,
) -> Option<usize> {
    let reference = deployment.system.as_ref()?;
    if let ReferenceForm::Declared { package, name } = &reference.form
        && let Some(index) = lookup.system(&deployment.package, package.as_deref(), name)
    {
        return Some(index);
    }
    check.error(
        DiagCode::RSDL_704,
        reference.site,
        format!(
            "`{}` names no declared `system` — a deployment is `for` the workspace's system \
             (rsdl reference §3.4)",
            reference.text()
        ),
    );
    None
}

/// Places the closure in `deployment`, which is `for` the closure's system
/// (rsdl §9).
fn place_one(
    lookup: &Lookup,
    system: &CheckedSystem,
    deployment: &DeploymentDecl,
    closure: &Closure,
    check: &mut DeploymentCheck,
) -> Vec<Placement> {
    let system_name = &system.systems[0].name.name;
    let mut placements: Vec<Placement> = Vec::new();
    let mut first_machine: HashMap<(usize, String), usize> = HashMap::new();
    for (machine_index, machine) in deployment.machines.iter().enumerate() {
        for (line_index, line) in machine.members.iter().enumerate() {
            let reference = &line.reference;
            let Some(target) = member_target(
                lookup,
                system,
                &deployment.package,
                reference,
                check.reporter,
            ) else {
                continue;
            };
            let Some(component_index) = closure.component_for(&target) else {
                let why = if target == MemberTarget::Unknown {
                    "resolves to nothing".to_string()
                } else {
                    format!("is outside the closure of `{system_name}`")
                };
                check.error(
                    DiagCode::RSDL_702,
                    reference.site,
                    format!(
                        "`{}` {why} — a machine places closure components, their instances and \
                         the lone services the system lists (rsdl reference §9)",
                        reference.text()
                    ),
                );
                continue;
            };
            let component = &closure.components[component_index];
            let instances = match target {
                MemberTarget::Instance(_, instance) => {
                    if !component.instances.contains(&instance) {
                        check.error(
                            DiagCode::RSDL_702,
                            reference.site,
                            format!(
                                "`{}` declares no instance `{instance}` (rsdl reference §7, §9)",
                                component.id.text()
                            ),
                        );
                        continue;
                    }
                    vec![instance]
                }
                _ => component.instances.clone(),
            };
            if machine.external && !component.external {
                check.error(
                    DiagCode::RSDL_707,
                    reference.site,
                    format!(
                        "`{}` is an `external` machine and hosts external components only, and \
                         `{}` is implemented in this workspace (rsdl reference §9)",
                        machine.name.name,
                        component.id.text()
                    ),
                );
            }
            for instance in instances {
                if let Some(&first) = first_machine.get(&(component_index, instance.clone())) {
                    check.error(
                        DiagCode::RSDL_706,
                        reference.site,
                        format!(
                            "`{}.{instance}` is placed twice in `{}` — first on `{}` (rsdl \
                             reference §9)",
                            component.id.text(),
                            deployment.name.name,
                            deployment.machines[first].name.name
                        ),
                    );
                    continue;
                }
                first_machine.insert((component_index, instance.clone()), machine_index);
                placements.push(Placement {
                    component: component_index,
                    instance,
                    machine: machine_index,
                    line: line_index,
                });
            }
        }
    }
    for (component_index, component) in closure.components.iter().enumerate() {
        for instance in &component.instances {
            if !first_machine.contains_key(&(component_index, instance.clone())) {
                check.error(
                    DiagCode::RSDL_701,
                    deployment.name.site,
                    format!(
                        "`{}.{instance}` is placed on no machine of `{}` — every instance of the \
                         closure is on exactly one machine per deployment (rsdl reference §9)",
                        component.id.text(),
                        deployment.name.name
                    ),
                );
            }
        }
    }
    placements
}
