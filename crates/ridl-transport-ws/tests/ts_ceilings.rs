//! The TypeScript client of the binding, `packages/ridl-transport-ws`, holds
//! the three default ceilings of `ridl_rt::contract` as constants of its
//! `src/frame.ts`. This test reads that file and compares each value with the
//! crate's, so the two sides cannot drift apart.

use ridl_rt::contract::{CALL_CEILING, FIXED_CEILING, SIGNAL_EVENT_CEILING};
use ridl_transport_ws::frame::{BLOCK_BYTES, FRAME_VERSION, SUBPROTOCOL, SUBPROTOCOL_V1};

const FRAME_TS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packages/ridl-transport-ws/src/frame.ts"
);

const BLOCKS_TS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packages/ridl-transport-ws/src/blocks.ts"
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

/// The value of `export const <name> = "<text>";` in `frame.ts`, without its
/// quotes.
fn ts_string(source: &str, name: &str) -> String {
    let prefix = format!("export const {name} = ");
    let line = source
        .lines()
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("frame.ts declares `{prefix}\"<text>\";`"));
    let value = line[prefix.len()..].trim_end_matches(';');
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or_else(|| panic!("`{name}` in frame.ts is a string, not `{value}`"))
        .to_string()
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

#[test]
fn the_typescript_client_holds_the_block_size_the_version_and_the_subprotocols() {
    let source =
        std::fs::read_to_string(FRAME_TS).expect("read packages/ridl-transport-ws/src/frame.ts");
    // `frame.ts` re-exports `BLOCK_BYTES`, which `blocks.ts` declares.
    let blocks =
        std::fs::read_to_string(BLOCKS_TS).expect("read packages/ridl-transport-ws/src/blocks.ts");
    assert_eq!(ts_constant(&blocks, "BLOCK_BYTES"), BLOCK_BYTES);
    assert_eq!(
        ts_constant(&source, "FRAME_VERSION"),
        FRAME_VERSION as usize
    );
    assert_eq!(ts_string(&source, "SUBPROTOCOL"), SUBPROTOCOL);
    assert_eq!(ts_string(&source, "SUBPROTOCOL_V1"), SUBPROTOCOL_V1);
}
