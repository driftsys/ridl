//! The deployment verifier refuses what it cannot read, and the catalog
//! verifier refuses a deployment descriptor.

mod deployment_common;

use deployment_common::{catalog_bytes, deployment, valid};
use ridl_descriptor::VerifyError;
use ridl_descriptor::deployment::{SCHEMA_VERSION, finish, verify};

#[test]
fn a_short_buffer_is_refused() {
    assert!(matches!(verify(&[0; 7]), Err(VerifyError::TooShort(7))));
}

#[test]
fn a_catalog_descriptor_is_refused() {
    let err = verify(&catalog_bytes()).unwrap_err();
    assert!(matches!(err, VerifyError::WrongIdentifier(id) if &id == b"RDLC"));
}

#[test]
fn a_wrong_version_is_refused() {
    let bytes = finish(&deployment(2, catalog_bytes()));
    assert!(matches!(verify(&bytes), Err(VerifyError::WrongVersion(2))));
}

#[test]
fn a_truncated_buffer_is_refused() {
    let bytes = finish(&valid());
    let cut = &bytes[..bytes.len() - 8];
    assert!(matches!(verify(cut), Err(VerifyError::Invalid(_))));
}

#[test]
fn an_embedded_catalog_that_is_not_a_catalog_is_refused() {
    let bytes = finish(&deployment(SCHEMA_VERSION, vec![0u8; 16]));
    match verify(&bytes) {
        Err(VerifyError::EmbeddedCatalog { catalog, source }) => {
            assert_eq!(catalog, "veh");
            assert!(matches!(*source, VerifyError::WrongIdentifier(_)));
        }
        other => panic!("expected an embedded catalog error, got {other:?}"),
    }
}

#[test]
fn the_catalog_verifier_refuses_a_deployment_descriptor() {
    let bytes = finish(&valid());
    let err = ridl_descriptor::verify(&bytes).unwrap_err();
    assert!(matches!(err, VerifyError::WrongIdentifier(id) if &id == b"RDLD"));
}
