//! RIDL intermediate representation, version 0.
//!
//! The types are generated from the protobuf schema at
//! `proto/ridl/ir/v0/ir.proto` by `build.rs` (protox + prost-build) and
//! re-exported at the crate root. This is the walking-skeleton subset: named
//! scalar types (`TypeDef`) with optional units and ranges, and value
//! constants (`ConstDef`), grouped into a `Module`.

mod v0 {
    include!(concat!(env!("OUT_DIR"), "/ridl.ir.v0.rs"));
}

pub use v0::{ConstDef, Module, Range, TypeDef};

#[cfg(test)]
mod round_trip {
    use crate::{ConstDef, Module, Range, TypeDef};
    use prost::Message;

    fn fixture() -> Module {
        Module {
            name: "vehicle".to_string(),
            types: vec![TypeDef {
                name: "Speed".to_string(),
                unit: "km/h".to_string(),
                range: Some(Range {
                    min: 0.0,
                    max: 250.0,
                    step: 0.5,
                }),
            }],
            consts: vec![ConstDef {
                name: "MAX_SPEED".to_string(),
                type_name: "Speed".to_string(),
                value: 250.0,
            }],
        }
    }

    #[test]
    fn protobuf_round_trip_preserves_module() {
        let module = fixture();

        let mut buf = Vec::new();
        module.encode(&mut buf).expect("encode must succeed");
        let decoded = Module::decode(buf.as_slice()).expect("decode must succeed");

        assert_eq!(module, decoded);
    }

    #[test]
    fn json_rendering_contains_type_name() {
        let module = fixture();

        let json = serde_json::to_string(&module).expect("json serialization must succeed");

        assert!(
            json.contains(r#""name":"Speed""#),
            "json debug rendering must include the Speed type name, got: {json}"
        );
    }
}
