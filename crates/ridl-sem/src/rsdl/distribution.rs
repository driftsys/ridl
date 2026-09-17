//! Distributions (rsdl reference §3.3): membership, tier, and the dependency
//! fact.
//!
//! Every rule of §3.3 is stated over the closure — a member line names a
//! closure component or a lone service the system lists — so the rules run
//! only when a closure exists. A workspace with no distribution derives no
//! installation and no dependency (§3.3), and draws none of these diagnostics.
//! RSDL-901 is checked over the dependency fact (§13), which this pass derives
//! from the resolved `requires` lines.

use std::collections::BTreeSet;

use ridl_core::diag::DiagCode;

use super::closure::{Closure, Lookup, MemberTarget, member_target};
use super::{CheckedSystem, Reporter, Tier};

/// The distribution facts of the closure (rsdl §3.3, §13).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DistributionFacts {
    /// Parallel to `Closure::components`: the distribution holding each
    /// component, as an index into `CheckedSystem::distributions`. `None` for
    /// an external component, and for an implemented one in no distribution
    /// (RSDL-904). A component listed by two keeps the first (RSDL-905).
    pub membership: Vec<Option<usize>>,
    /// Parallel to `membership`: the member line of the holding distribution
    /// that lists each component, as an index into
    /// `DistributionDecl::members`; its backend keys are the line's (rsdl §13).
    pub member_lines: Vec<Option<usize>>,
    /// The dependency fact (rsdl §13): each pair once, in `(from, to)` order.
    pub dependencies: Vec<DistributionDependency>,
}

/// `from` depends on `to`: a component in `from` requires an interface whose
/// owning service a component in `to` offers (rsdl §13). Both are indexes into
/// `CheckedSystem::distributions`, and they differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DistributionDependency {
    pub from: usize,
    pub to: usize,
}

/// Checks every distribution against the closure and derives the facts, or
/// `None` when the workspace declares no distribution (rsdl §3.3).
pub(super) fn distribute(
    lookup: &Lookup,
    system: &CheckedSystem,
    closure: &Closure,
    reporter: &mut Reporter,
) -> Option<DistributionFacts> {
    if system.distributions.is_empty() {
        return None;
    }
    let mut membership: Vec<Option<usize>> = vec![None; closure.components.len()];
    let mut member_lines: Vec<Option<usize>> = vec![None; closure.components.len()];
    for (index, distribution) in system.distributions.iter().enumerate() {
        let mut listed = Vec::new();
        for (position, line) in distribution.members.iter().enumerate() {
            let reference = &line.reference;
            let Some(target) =
                member_target(lookup, system, &distribution.package, reference, reporter)
            else {
                continue;
            };
            let component = match target {
                MemberTarget::Component(_) | MemberTarget::Implicit(_) => {
                    closure.component_for(&target)
                }
                MemberTarget::Instance(..) | MemberTarget::Unknown => None,
            };
            let Some(component) = component else {
                reporter.error(
                    DiagCode::RSDL_903,
                    reference.site,
                    format!(
                        "`{}` is not a component of the closure of `{}` — a distribution lists \
                         closure components and the lone services the system lists (rsdl \
                         reference §3.3)",
                        reference.text(),
                        system.systems[0].name.name
                    ),
                );
                continue;
            };
            let name = closure.components[component].id.text();
            if listed.contains(&component) {
                reporter.error(
                    DiagCode::RSDL_906,
                    reference.site,
                    format!(
                        "`{name}` is listed twice in `{}` (rsdl reference §3.3)",
                        distribution.name.name
                    ),
                );
                continue;
            }
            listed.push(component);
            if closure.components[component].external {
                reporter.error(
                    DiagCode::RSDL_907,
                    reference.site,
                    format!(
                        "`{name}` is external, and an external component is in no distribution \
                         (rsdl reference §3.3)"
                    ),
                );
                continue;
            }
            match membership[component] {
                Some(first) => reporter.error(
                    DiagCode::RSDL_905,
                    reference.site,
                    format!(
                        "`{name}` is already in the distribution `{}` — an implemented closure \
                         component is in exactly one (rsdl reference §3.3)",
                        system.distributions[first].name.name
                    ),
                ),
                None => {
                    membership[component] = Some(index);
                    member_lines[component] = Some(position);
                }
            }
        }
    }
    for (component, held) in closure.components.iter().zip(&membership) {
        if !component.external && held.is_none() {
            reporter.error(
                DiagCode::RSDL_904,
                component.site,
                format!(
                    "`{}` is in no distribution — when the workspace declares one, every \
                     implemented closure component is in exactly one (rsdl reference §3.3)",
                    component.id.text()
                ),
            );
        }
    }
    let dependencies: BTreeSet<DistributionDependency> = closure
        .requires
        .iter()
        .filter_map(|require| {
            let from = membership[require.consumer]?;
            let to = membership[require.producer]?;
            (from != to).then_some(DistributionDependency { from, to })
        })
        .collect();
    for dependency in &dependencies {
        let from = &system.distributions[dependency.from];
        let to = &system.distributions[dependency.to];
        if from.tier != Some(Tier::Platform) || to.tier != Some(Tier::Application) {
            continue;
        }
        let witness = closure
            .requires
            .iter()
            .find(|require| {
                membership[require.consumer] == Some(dependency.from)
                    && membership[require.producer] == Some(dependency.to)
            })
            .expect("a dependency is derived from a resolved requires line");
        reporter.error(
            DiagCode::RSDL_901,
            from.name.site,
            format!(
                "`{}` has tier `PLATFORM` and depends on `{}`, which has tier `APPLICATION`: \
                 `{}` requires `{}`, which `{}` offers — tier inversion (rsdl reference §3.3)",
                from.name.name,
                to.name.name,
                closure.components[witness.consumer].id.text(),
                witness.interface.text(),
                closure.components[witness.producer].id.text()
            ),
        );
    }
    Some(DistributionFacts {
        membership,
        member_lines,
        dependencies: dependencies.into_iter().collect(),
    })
}
