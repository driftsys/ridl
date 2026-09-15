//! Collects the rsdl declarations of every `.rsdl` file into the model (rsdl
//! reference §3) and reads each reference by case (rsdl §4).
//!
//! A declaration or a line the parser recovered without a name or a reference
//! is skipped: its parse error is already reported, and a model entry with no
//! name would only draw a second diagnostic for the same mistake.

use ridl_core::db::{InputFile, profile_of_path};
use ridl_core::package::Workspace;
use ridl_syntax::Profile;
use ridl_syntax::ast::{self, AstNode, ComponentLineKind};

use super::attrs::{self, AttrSite};
use super::{
    CheckedSystem, ComponentDecl, DeploymentDecl, DistributionDecl, MachineDecl, MemberRef, Named,
    Reference, ReferenceForm, Reporter, Site, SystemDecl, UNIT_INSTANCE, is_lower_camel,
    is_lowercase_segment, is_upper_camel,
};
use crate::resolve::{qualified_segments, source_file};

/// Walks every `.rsdl` file of `ws`, in package-then-file order.
pub(super) fn collect(
    db: &dyn salsa::Database,
    ws: Workspace,
    reporter: &mut Reporter,
) -> CheckedSystem {
    let mut system = CheckedSystem::default();
    for package in ws.packages(db) {
        let package_name = package.name(db).clone();
        for file in package.files(db) {
            if profile_of_path(file.path(db)) != Profile::Rsdl {
                continue;
            }
            let file = *file;
            let source = source_file(db, file);
            for decl in source.systems() {
                if let Some(decl) = system_decl(&decl, file, &package_name, reporter) {
                    system.systems.push(decl);
                }
            }
            for decl in source.components() {
                if let Some(decl) = component_decl(&decl, file, &package_name, reporter) {
                    system.components.push(decl);
                }
            }
            for decl in source.distributions() {
                if let Some(decl) = distribution_decl(&decl, file, &package_name, reporter) {
                    system.distributions.push(decl);
                }
            }
            for decl in source.deployments() {
                if let Some(decl) = deployment_decl(&decl, file, &package_name, reporter) {
                    system.deployments.push(decl);
                }
            }
        }
    }
    system
}

fn system_decl(
    decl: &ast::SystemDef,
    file: InputFile,
    package: &str,
    reporter: &mut Reporter,
) -> Option<SystemDecl> {
    let name = named(decl.name(), file)?;
    let read = attrs::read(decl.attr_block(), AttrSite::System, file, reporter);
    let members = decl
        .lines()
        .filter_map(|line| member_ref(line.reference(), line.attr_block(), file, reporter))
        .collect();
    Some(SystemDecl {
        name,
        package: package.to_string(),
        members,
        attrs: read.attrs,
    })
}

fn component_decl(
    decl: &ast::ComponentDef,
    file: InputFile,
    package: &str,
    reporter: &mut Reporter,
) -> Option<ComponentDecl> {
    let name = named(decl.name(), file)?;
    let read = attrs::read(decl.attr_block(), AttrSite::Component, file, reporter);
    let mut offers = Vec::new();
    let mut requires = Vec::new();
    for line in decl.lines() {
        let Some(member) = member_ref(line.reference(), line.attr_block(), file, reporter) else {
            continue;
        };
        match line.kind() {
            Some(ComponentLineKind::Offers) => offers.push(member),
            Some(ComponentLineKind::Requires) => requires.push(member),
            None => {}
        }
    }
    Some(ComponentDecl {
        name,
        package: package.to_string(),
        offers,
        requires,
        instances: read.instances,
        external: read.external,
        attrs: read.attrs,
    })
}

fn distribution_decl(
    decl: &ast::DistributionDef,
    file: InputFile,
    package: &str,
    reporter: &mut Reporter,
) -> Option<DistributionDecl> {
    let name = named(decl.name(), file)?;
    let read = attrs::read(decl.attr_block(), AttrSite::Distribution, file, reporter);
    let members = decl
        .lines()
        .filter_map(|line| member_ref(line.reference(), line.attr_block(), file, reporter))
        .collect();
    Some(DistributionDecl {
        name,
        package: package.to_string(),
        members,
        tier: read.tier,
        attrs: read.attrs,
    })
}

fn deployment_decl(
    decl: &ast::DeploymentDef,
    file: InputFile,
    package: &str,
    reporter: &mut Reporter,
) -> Option<DeploymentDecl> {
    let name = named(decl.name(), file)?;
    let system = decl
        .system()
        .and_then(|reference| read_reference(&reference, file));
    let read = attrs::read(decl.attr_block(), AttrSite::Deployment, file, reporter);
    let machines = decl
        .machines()
        .filter_map(|machine| machine_decl(&machine, file, reporter))
        .collect();
    Some(DeploymentDecl {
        name,
        package: package.to_string(),
        system,
        machines,
        attrs: read.attrs,
    })
}

fn machine_decl(
    decl: &ast::MachineDef,
    file: InputFile,
    reporter: &mut Reporter,
) -> Option<MachineDecl> {
    let name = named(decl.name(), file)?;
    let read = attrs::read(decl.attr_block(), AttrSite::Machine, file, reporter);
    let members = decl
        .lines()
        .filter_map(|line| member_ref(line.reference(), line.attr_block(), file, reporter))
        .collect();
    Some(MachineDecl {
        name,
        members,
        external: read.external,
        attrs: read.attrs,
    })
}

/// A declared name and its site, or `None` when the parser recovered the
/// declaration without one.
fn named(name: Option<ast::Name>, file: InputFile) -> Option<Named> {
    let name = name?;
    let token = name.ident_token()?;
    Some(Named {
        name: token.text().to_string(),
        site: Site {
            file,
            range: name.syntax().text_range(),
        },
    })
}

/// One body line: its reference and the backend keys of its attribute block.
fn member_ref(
    reference: Option<ast::Reference>,
    block: Option<ast::AttrBlock>,
    file: InputFile,
    reporter: &mut Reporter,
) -> Option<MemberRef> {
    let reference = read_reference(&reference?, file)?;
    let backend_keys = attrs::read(block, AttrSite::Line, file, reporter)
        .attrs
        .backend_keys;
    Some(MemberRef {
        reference,
        backend_keys,
    })
}

/// A reference's segments, site and form, or `None` when the parser recovered
/// it with a dangling `.` (FORM-101 is already reported).
fn read_reference(reference: &ast::Reference, file: InputFile) -> Option<Reference> {
    let qualified = reference.qualified_name()?;
    let segments = qualified_segments(&qualified);
    if segments.iter().any(String::is_empty) {
        return None;
    }
    let form = form_of(&segments);
    Some(Reference {
        segments,
        site: Site {
            file,
            range: reference.syntax().text_range(),
        },
        form,
    })
}

/// Reads a reference's role from the case of its segments (rsdl §4): the
/// segments before the first upper-case one are the package path, that
/// segment names a component (or the system, or an interface), and one
/// camelCase segment after it names an instance. A reference of lowercase
/// segments only is a service.
fn form_of(segments: &[String]) -> ReferenceForm {
    let Some(first_upper) = segments
        .iter()
        .position(|segment| segment.starts_with(|c: char| c.is_ascii_uppercase()))
    else {
        return if segments.iter().all(|segment| is_lowercase_segment(segment)) {
            ReferenceForm::Service {
                name: segments.join("."),
            }
        } else {
            ReferenceForm::Unreadable
        };
    };
    let (package, rest) = segments.split_at(first_upper);
    if !package.iter().all(|segment| is_lowercase_segment(segment)) {
        return ReferenceForm::Unreadable;
    }
    let package = (!package.is_empty()).then(|| package.join("."));
    match rest {
        [name] if is_upper_camel(name) => ReferenceForm::Declared {
            package,
            name: name.clone(),
        },
        [name, instance]
            if is_upper_camel(name) && (is_lower_camel(instance) || instance == UNIT_INSTANCE) =>
        {
            ReferenceForm::Instance {
                package,
                component: name.clone(),
                instance: instance.clone(),
            }
        }
        _ => ReferenceForm::Unreadable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(text: &str) -> ReferenceForm {
        let segments: Vec<String> = text.split('.').map(str::to_string).collect();
        form_of(&segments)
    }

    fn declared(package: Option<&str>, name: &str) -> ReferenceForm {
        ReferenceForm::Declared {
            package: package.map(str::to_string),
            name: name.to_string(),
        }
    }

    fn instance(package: Option<&str>, component: &str, instance: &str) -> ReferenceForm {
        ReferenceForm::Instance {
            package: package.map(str::to_string),
            component: component.to_string(),
            instance: instance.to_string(),
        }
    }

    /// The five rows of the rsdl §4 reference table, read by case.
    #[test]
    fn a_reference_reads_its_role_from_case() {
        assert_eq!(form("Cruise"), declared(None, "Cruise"));
        assert_eq!(form("Cruise.primary"), instance(None, "Cruise", "primary"));
        assert_eq!(
            form("veh.topology.Cruise"),
            declared(Some("veh.topology"), "Cruise")
        );
        assert_eq!(
            form("veh.topology.Cruise.primary"),
            instance(Some("veh.topology"), "Cruise", "primary")
        );
        assert_eq!(
            form("veh.diag.access"),
            ReferenceForm::Service {
                name: "veh.diag.access".to_string()
            }
        );
    }

    /// `Cruise.Unit` reads as an instance, so RSDL-307 can report it (rsdl §7);
    /// every shape no table row names is unreadable.
    #[test]
    fn a_written_unit_is_an_instance_and_other_shapes_are_unreadable() {
        assert_eq!(form("Cruise.Unit"), instance(None, "Cruise", "Unit"));
        for text in [
            "Cruise.Backup",
            "Cruise.primary.extra",
            "veh.Adas.Cruise",
            "veh.adas.laneAssist",
            "ASIL_B.x",
            "Cruise_2",
        ] {
            assert_eq!(form(text), ReferenceForm::Unreadable, "`{text}`");
        }
    }
}
