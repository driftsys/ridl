//! The interface numbers (rsdl note D-7), copied from the IR. `ridl-sem`
//! folds `interfaces.lock` into `Interface.number` and
//! `Interface.provisional` for every shape, declared or inline, so the
//! descriptor reads them and computes no numbering of its own. A number of
//! 0 never reaches a checked package: here it is an internal error, not data.

use ridl_ir::v2::{InterfaceShape, Package, members_of_unit};

/// One interface's number and whether a lock froze it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Numbered {
    pub name: String,
    pub number: u32,
    pub provisional: bool,
}

/// An interface shape whose IR number is 0: the package was not numbered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZeroNumber(pub String);

/// Every interface shape of the packages of `unit`, each with its package
/// and its catalog name, in (number, catalog name) order. The packages are
/// selected by [`ridl_ir::v2::unit_of`], and a package named twice is read
/// once.
pub(crate) fn unit_shapes<'a>(
    unit: &str,
    packages: &[&'a Package],
) -> Vec<(&'a Package, InterfaceShape<'a>, String)> {
    let mut seen: Vec<&str> = Vec::new();
    let mut shapes: Vec<(&Package, InterfaceShape<'_>, String)> = Vec::new();
    for package in members_of_unit(unit, packages) {
        if seen.contains(&package.name.as_str()) {
            continue;
        }
        seen.push(&package.name);
        shapes.extend(package.shapes().map(|shape| {
            let name = package.catalog_name(&shape);
            (package, shape, name)
        }));
    }
    shapes.sort_by(|a, b| (a.1.interface.number, &a.2).cmp(&(b.1.interface.number, &b.2)));
    shapes
}

/// The zero-number rule: `Err` naming the first shape of `shapes` whose IR
/// number is 0, `Ok` when every shape is numbered.
pub(crate) fn first_zero(
    shapes: &[(&Package, InterfaceShape<'_>, String)],
) -> Result<(), ZeroNumber> {
    match shapes
        .iter()
        .find(|(_, shape, _)| shape.interface.number == 0)
    {
        Some((_, _, name)) => Err(ZeroNumber(name.clone())),
        None => Ok(()),
    }
}

/// Every interface shape of the packages of `unit` under its catalog name, in
/// (number, name) order, with the number and the provisional flag the IR
/// carries.
pub fn numbered_shapes(unit: &str, packages: &[&Package]) -> Result<Vec<Numbered>, ZeroNumber> {
    let shapes = unit_shapes(unit, packages);
    first_zero(&shapes)?;
    Ok(shapes
        .into_iter()
        .map(|(_, shape, name)| Numbered {
            name,
            number: shape.interface.number,
            provisional: shape.interface.provisional,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_ir::v2::{Interface, Service, ServiceShape, service_shape};

    fn interface(name: &str, number: u32, provisional: bool) -> Interface {
        Interface {
            name: name.to_owned(),
            number,
            provisional,
            ..Default::default()
        }
    }

    fn package(interfaces: Vec<Interface>) -> Package {
        Package {
            name: "p".to_owned(),
            interfaces,
            ..Default::default()
        }
    }

    /// A `service` with an inline body: its `Interface` lives in the shape
    /// list, not in `Package::interfaces`, and its own `name` is empty.
    fn inline_service(name: &str, number: u32) -> Service {
        Service {
            name: name.to_owned(),
            shapes: vec![ServiceShape {
                kind: Some(service_shape::Kind::Inline(interface("", number, true))),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn numbers_and_flags_are_copied_in_number_order() {
        let numbered = numbered_shapes(
            "p",
            &[&package(vec![
                interface("B", 7, false),
                interface("A", 2, true),
            ])],
        )
        .unwrap();
        assert_eq!(
            numbered,
            vec![
                Numbered {
                    name: "A".to_owned(),
                    number: 2,
                    provisional: true
                },
                Numbered {
                    name: "B".to_owned(),
                    number: 7,
                    provisional: false
                },
            ]
        );
    }

    #[test]
    fn an_inline_service_shape_is_numbered_under_the_service_name() {
        let mut package = package(vec![interface("A", 1, false)]);
        package.services.push(inline_service("p.hvac", 3));
        assert_eq!(
            numbered_shapes("p", &[&package]).unwrap(),
            vec![
                Numbered {
                    name: "A".to_owned(),
                    number: 1,
                    provisional: false
                },
                Numbered {
                    name: "p.hvac".to_owned(),
                    number: 3,
                    provisional: true
                },
            ]
        );
    }

    #[test]
    fn a_zero_number_is_an_internal_error() {
        let package = package(vec![interface("A", 1, false), interface("Z", 0, true)]);
        assert_eq!(
            numbered_shapes("p", &[&package]),
            Err(ZeroNumber("Z".to_owned()))
        );
    }

    #[test]
    fn a_package_without_interfaces_numbers_nothing() {
        assert!(
            numbered_shapes("p", &[&package(vec![])])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_unit_of_two_packages_is_qualified_and_sorted_by_number_then_name() {
        let mut cluster = package(vec![interface("Speed", 1, false)]);
        cluster.name = "u.cluster".to_owned();
        cluster.unit = "u".to_owned();
        let mut root = package(vec![interface("Session", 2, true)]);
        root.name = "u".to_owned();
        root.services.push(inline_service("veh.x", 3));
        let mut other = package(vec![interface("Other", 1, false)]);
        other.name = "w".to_owned();
        // Its name extends the unit's, but it is the root of its own unit.
        let mut sibling = package(vec![interface("Sib", 4, false)]);
        sibling.name = "u.sib".to_owned();
        sibling.unit = "u.sib".to_owned();
        let names: Vec<String> = numbered_shapes("u", &[&root, &cluster, &other, &sibling, &root])
            .unwrap()
            .into_iter()
            .map(|n| n.name)
            .collect();
        assert_eq!(names, ["cluster.Speed", "Session", "veh.x"]);
    }
}
