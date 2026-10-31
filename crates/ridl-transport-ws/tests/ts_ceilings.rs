//! The TypeScript client of the binding, `packages/ridl-transport-ws`, holds
//! the three default ceilings of `ridl_rt::contract` as constants of its
//! `src/frame.ts`. This test reads that file and compares each value with the
//! crate's, so the two sides cannot drift apart.

use ridl_rt::contract::{CALL_CEILING, FIXED_CEILING, SIGNAL_EVENT_CEILING};

const FRAME_TS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packages/ridl-transport-ws/src/frame.ts"
);

/// The value of `export const <name> = <number>;` in `frame.ts`. A numeric
/// separator (`16_128`) is accepted, as TypeScript accepts it.
fn ts_constant(source: &str, name: &str) -> usize {
    let prefix = format!("export const {name} = ");
    let line = source
        .lines()
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("frame.ts declares `{prefix}<number>;`"));
    let value = line[prefix.len()..].trim_end_matches(';').replace('_', "");
    value
        .parse()
        .unwrap_or_else(|_| panic!("`{name}` in frame.ts is a number, not `{value}`"))
}

#[test]
fn the_typescript_client_holds_the_default_ceilings_of_ridl_rt() {
    let source =
        std::fs::read_to_string(FRAME_TS).expect("read packages/ridl-transport-ws/src/frame.ts");
    assert_eq!(
        ts_constant(&source, "SIGNAL_EVENT_CEILING"),
        SIGNAL_EVENT_CEILING
    );
    assert_eq!(ts_constant(&source, "CALL_CEILING"), CALL_CEILING);
    assert_eq!(ts_constant(&source, "FIXED_CEILING"), FIXED_CEILING);
}
