//! The deployment descriptor reads back what was written.

mod deployment_common;

use deployment_common::{attributes, valid};
use ridl_descriptor::deployment::{
    Crossing, Deployment, FILE_IDENTIFIER, SurfaceDirection, finish, verify,
};

#[test]
fn a_deployment_round_trips() {
    let bytes = finish(&valid());
    let view = verify(&bytes).expect("verifies");

    // The whole owned value comes back, which covers every field of every
    // table, including the attribute value nested two lists deep.
    let back = Deployment::try_from(view).expect("converts to the owned form");
    assert_eq!(back, valid());

    // The borrowed accessors agree with the owned value.
    assert_eq!(view.system().unwrap(), "veh.Vehicle");
    assert_eq!(view.name().unwrap(), "Prod");
    assert_eq!(view.package().unwrap(), "veh");
    let link = view.links().unwrap().get(0).unwrap().unwrap();
    assert_eq!(link.crossing().unwrap(), Crossing::OffBoard);
    assert_eq!(link.producer().unwrap().component().unwrap(), "Climate");
    let surface = view.surface().unwrap().get(0).unwrap().unwrap();
    assert_eq!(
        surface.direction().unwrap(),
        SurfaceDirection::ExternalOffers
    );

    let nested = view.attributes().unwrap().get(2).unwrap().unwrap();
    let outer = nested.value().unwrap().expect("a value");
    let inner = outer
        .list()
        .unwrap()
        .expect("a list")
        .get(1)
        .unwrap()
        .unwrap();
    let deep = inner
        .list()
        .unwrap()
        .expect("a list")
        .get(0)
        .unwrap()
        .unwrap();
    assert_eq!(deep.scalar().unwrap(), Some("deep"));
    assert_eq!(attributes().len(), 3);
}

#[test]
fn the_identifier_is_at_bytes_4_to_8() {
    let bytes = finish(&valid());
    assert_eq!(&bytes[4..8], b"RDLD");
    assert_eq!(FILE_IDENTIFIER, *b"RDLD");
}
