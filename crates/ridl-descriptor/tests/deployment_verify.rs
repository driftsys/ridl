//! The deployment verifier refuses what it cannot read, and the catalog
//! verifier refuses a deployment descriptor.

mod deployment_common;

use deployment_common::{catalog_bytes, deployment, valid};
use ridl_descriptor::deployment::{
    Attribute, AttributeValue, FILE_IDENTIFIER, MAX_ATTRIBUTE_DEPTH, SCHEMA_VERSION, VerifyError,
    finish, verify,
};

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
            assert!(matches!(
                *source,
                ridl_descriptor::VerifyError::WrongIdentifier(_)
            ));
        }
        other => panic!("expected an embedded catalog error, got {other:?}"),
    }
}

#[test]
fn a_stored_hash_that_differs_from_the_embedded_catalog_is_refused() {
    let mut owned = deployment(SCHEMA_VERSION, catalog_bytes());
    owned.catalogs[0].hash[0] ^= 1;
    match verify(&finish(&owned)) {
        Err(VerifyError::CatalogHashMismatch { catalog }) => assert_eq!(catalog, "veh"),
        other => panic!("expected a hash mismatch, got {other:?}"),
    }
}

#[test]
fn the_catalog_verifier_refuses_a_deployment_descriptor() {
    let bytes = finish(&valid());
    let err = ridl_descriptor::verify(&bytes).unwrap_err();
    assert!(matches!(err, ridl_descriptor::VerifyError::WrongIdentifier(id) if &id == b"RDLD"));
}

#[test]
fn the_foreign_identifier_message_names_rdld() {
    let message = verify(&catalog_bytes()).unwrap_err().to_string();
    assert!(message.contains("RDLD"), "{message}");
    assert!(message.contains("RDLC"), "{message}");
    assert_eq!(FILE_IDENTIFIER, *b"RDLD");
}

/// A value whose lists nest `levels` deep, the innermost being a scalar.
fn nested(levels: usize) -> AttributeValue {
    let mut value = AttributeValue {
        scalar: Some("leaf".to_owned()),
        list: None,
    };
    for _ in 1..levels {
        value = AttributeValue {
            scalar: None,
            list: Some(vec![value]),
        };
    }
    value
}

fn with_value(value: AttributeValue) -> Vec<u8> {
    let mut d = valid();
    d.attributes = vec![Attribute {
        ns: "rust".to_owned(),
        key: "k".to_owned(),
        value: Some(Box::new(value)),
    }];
    finish(&d)
}

#[test]
fn a_list_nested_at_the_limit_is_accepted() {
    verify(&with_value(nested(MAX_ATTRIBUTE_DEPTH))).expect("verifies");
}

#[test]
fn a_list_nested_one_level_past_the_limit_is_refused() {
    let bytes = with_value(nested(MAX_ATTRIBUTE_DEPTH + 1));
    assert!(matches!(verify(&bytes), Err(VerifyError::TooDeep)));
}

#[test]
fn a_value_with_both_scalar_and_list_is_refused() {
    let both = AttributeValue {
        scalar: Some("x".to_owned()),
        list: Some(vec![]),
    };
    assert!(matches!(
        verify(&with_value(both)),
        Err(VerifyError::MalformedAttributeValue)
    ));
}

#[test]
fn a_value_with_neither_scalar_nor_list_is_refused() {
    let neither = AttributeValue {
        scalar: None,
        list: None,
    };
    assert!(matches!(
        verify(&with_value(neither)),
        Err(VerifyError::MalformedAttributeValue)
    ));
}

/// A buffer whose lists share child tables: two levels of `fan` references to
/// one table each. The buffer is a few kilobytes, and an unbounded walk makes
/// `fan * fan` visits.
#[test]
fn lists_that_share_one_child_many_times_are_refused() {
    use planus::{Builder, Offset};
    use ridl_descriptor::deployment::{Deployment, EmbeddedCatalog};

    fn empty<T>() -> Vec<Offset<T>> {
        Vec::new()
    }

    let fan = 2_000;
    let mut builder = Builder::new();
    let leaf = AttributeValue::create(
        &mut builder,
        Some("leaf"),
        None::<Vec<Offset<AttributeValue>>>,
    );
    let middle = AttributeValue::create(&mut builder, None::<&str>, Some(vec![leaf; fan]));
    let top = AttributeValue::create(&mut builder, None::<&str>, Some(vec![middle; fan]));
    let attribute = Attribute::create(&mut builder, "rust", "k", Some(top));
    let catalog = EmbeddedCatalog::create(&mut builder, "veh", vec![7u8; 32], catalog_bytes());
    let root = Deployment::create(
        &mut builder,
        SCHEMA_VERSION,
        "veh.Vehicle",
        "Prod",
        "veh",
        vec![catalog],
        empty(),
        empty(),
        empty(),
        empty(),
        empty(),
        empty(),
        empty(),
        vec![attribute],
        empty(),
    );
    let mut bytes = builder.finish(root, Some(FILE_IDENTIFIER)).to_vec();
    // The header swap that `finish` performs.
    let from_byte_4 = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    bytes[0..4].copy_from_slice(&(from_byte_4 + 4).to_le_bytes());
    bytes[4..8].copy_from_slice(&FILE_IDENTIFIER);

    assert!(
        bytes.len() < 100_000,
        "the buffer stays small: {}",
        bytes.len()
    );
    let started = std::time::Instant::now();
    let result = verify(&bytes);
    assert!(matches!(result, Err(VerifyError::TooManyAttributeValues)));
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
}

/// A deployment that embeds two catalogs, `aaa` and `veh`, in that order.
fn two_catalogs(second: Vec<u8>, second_hash: Vec<u8>) -> Vec<u8> {
    let mut owned = valid();
    owned.catalogs.insert(
        0,
        ridl_descriptor::deployment::EmbeddedCatalog {
            name: "aaa".to_owned(),
            hash: vec![7u8; 32],
            bytes: catalog_bytes(),
        },
    );
    owned.catalogs[1].bytes = second;
    owned.catalogs[1].hash = second_hash;
    finish(&owned)
}

#[test]
fn a_second_embedded_catalog_that_is_not_a_catalog_is_refused() {
    let bytes = two_catalogs(vec![0u8; 16], vec![7u8; 32]);
    match verify(&bytes) {
        Err(VerifyError::EmbeddedCatalog { catalog, .. }) => assert_eq!(catalog, "veh"),
        other => panic!("expected an embedded catalog error, got {other:?}"),
    }
}

#[test]
fn a_second_embedded_catalog_with_a_wrong_stored_hash_is_refused() {
    let bytes = two_catalogs(catalog_bytes(), vec![1u8; 32]);
    match verify(&bytes) {
        Err(VerifyError::CatalogHashMismatch { catalog }) => assert_eq!(catalog, "veh"),
        other => panic!("expected a hash mismatch, got {other:?}"),
    }
}

/// Writes `value` over each byte of a valid buffer in turn and collects the
/// message of the error `verify` gives, if any.
fn refusals_after_overwriting(value: u8) -> Vec<String> {
    let bytes = finish(&valid());
    let mut messages = Vec::new();
    for at in 8..bytes.len() {
        let mut damaged = bytes.clone();
        damaged[at] = value;
        if let Err(error) = verify(&damaged) {
            messages.push(format!("{error:?}"));
        }
    }
    messages
}

#[test]
fn an_unknown_crossing_is_refused_by_verify() {
    let messages = refusals_after_overwriting(0x7f);
    assert!(
        messages
            .iter()
            .any(|message| message.contains("method: \"crossing\"")),
        "no overwritten byte was refused as a crossing"
    );
}

#[test]
fn an_unknown_surface_direction_is_refused_by_verify() {
    let messages = refusals_after_overwriting(0x7f);
    assert!(
        messages
            .iter()
            .any(|message| message.contains("method: \"direction\"")),
        "no overwritten byte was refused as a surface direction"
    );
}

/// The promise of `verify`: when it returns a view, reading every field of it
/// cannot fail. Overwriting any one byte must give either a refusal or a
/// buffer that converts to the owned form.
#[test]
fn a_verified_buffer_reads_in_full_whatever_byte_was_damaged() {
    let bytes = finish(&valid());
    for value in [0x00, 0x01, 0x7f, 0xff] {
        for at in 8..bytes.len() {
            let mut damaged = bytes.clone();
            damaged[at] = value;
            if let Ok(view) = verify(&damaged) {
                ridl_descriptor::deployment::Deployment::try_from(view).unwrap_or_else(|error| {
                    panic!("byte {at} set to {value:#x} verified but does not read: {error}")
                });
            }
        }
    }
}
