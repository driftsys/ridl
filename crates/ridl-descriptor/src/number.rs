//! The interface numbers (rsdl note D-7), copied from the IR. `ridl-sem`
//! folds `interfaces.lock` into `Interface.number` and
//! `Interface.provisional` for every shape, declared or inline, so the
//! descriptor reads them and computes no numbering of its own. A number of
//! 0 never reaches a checked package: here it is an internal error, not data.

use ridl_ir::v2::Package;

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

/// Every interface shape of `package` in [`Package::shapes`] order, with the
/// number and the provisional flag the IR carries.
pub fn numbered_shapes(package: &Package) -> Result<Vec<Numbered>, ZeroNumber> {
    package
        .shapes()
        .map(|shape| {
            if shape.interface.number == 0 {
                return Err(ZeroNumber(shape.name.to_owned()));
            }
            Ok(Numbered {
                name: shape.name.to_owned(),
                number: shape.interface.number,
                provisional: shape.interface.provisional,
            })
        })
        .collect()
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
    fn numbers_and_flags_are_copied_in_shape_order() {
        let numbered = numbered_shapes(&package(vec![
            interface("B", 7, false),
            interface("A", 2, true),
        ]))
        .unwrap();
        assert_eq!(
            numbered,
            vec![
                Numbered {
                    name: "B".to_owned(),
                    number: 7,
                    provisional: false
                },
                Numbered {
                    name: "A".to_owned(),
                    number: 2,
                    provisional: true
                },
            ]
        );
    }

    #[test]
    fn an_inline_service_shape_is_numbered_under_the_service_name() {
        let mut package = package(vec![interface("A", 1, false)]);
        package.services.push(inline_service("p.hvac", 3));
        assert_eq!(
            numbered_shapes(&package).unwrap(),
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
        assert_eq!(numbered_shapes(&package), Err(ZeroNumber("Z".to_owned())));
    }

    #[test]
    fn a_package_without_interfaces_numbers_nothing() {
        assert!(numbered_shapes(&package(vec![])).unwrap().is_empty());
    }
}
