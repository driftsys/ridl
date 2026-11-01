//! The TypeScript client of the binding, `packages/ridl-transport-ws`, holds
//! as constants of its `src/frame.ts` and `src/blocks.ts`: the three default
//! ceilings of `ridl_rt::contract`, the block size, the frame version and the
//! two subprotocol names of the crate's `frame` module. This test reads those
//! files and compares each value with the crate's, so the two sides cannot
//! drift apart.

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

/// The text after `export const <name> = ` in `source`, up to its `;`.
/// `file` names the file in a failure message.
fn ts_value<'a>(source: &'a str, file: &str, name: &str) -> &'a str {
    let prefix = format!("export const {name} = ");
    source
        .lines()
        .find_map(|line| line.strip_prefix(prefix.as_str()))
        .unwrap_or_else(|| panic!("{file} declares `{prefix}<value>;`"))
        .trim_end_matches(';')
}

/// The value of `export const <name> = <number>;` in `file`. A numeric
/// separator (`16_128`) is accepted, as TypeScript accepts it.
fn ts_constant(source: &str, file: &str, name: &str) -> usize {
    let value = ts_value(source, file, name).replace('_', "");
    value
        .parse()
        .unwrap_or_else(|_| panic!("`{name}` in {file} is a number, not `{value}`"))
}

/// The value of `export const <name> = "<text>";` in `file`, without its
/// quotes.
fn ts_string(source: &str, file: &str, name: &str) -> String {
    let value = ts_value(source, file, name);
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or_else(|| panic!("`{name}` in {file} is a string, not `{value}`"))
        .to_string()
}

#[test]
fn the_typescript_client_holds_the_default_ceilings_of_ridl_rt() {
    let source =
        std::fs::read_to_string(FRAME_TS).expect("read packages/ridl-transport-ws/src/frame.ts");
    assert_eq!(
        ts_constant(&source, "frame.ts", "SIGNAL_EVENT_CEILING"),
        SIGNAL_EVENT_CEILING
    );
    assert_eq!(
        ts_constant(&source, "frame.ts", "CALL_CEILING"),
        CALL_CEILING
    );
    assert_eq!(
        ts_constant(&source, "frame.ts", "FIXED_CEILING"),
        FIXED_CEILING
    );
}

#[test]
fn the_typescript_client_holds_the_block_size_the_version_and_the_subprotocols() {
    let source =
        std::fs::read_to_string(FRAME_TS).expect("read packages/ridl-transport-ws/src/frame.ts");
    // `frame.ts` re-exports `BLOCK_BYTES`, which `blocks.ts` declares.
    let blocks =
        std::fs::read_to_string(BLOCKS_TS).expect("read packages/ridl-transport-ws/src/blocks.ts");
    assert_eq!(
        ts_constant(&blocks, "blocks.ts", "BLOCK_BYTES"),
        BLOCK_BYTES
    );
    assert_eq!(
        ts_constant(&source, "frame.ts", "FRAME_VERSION"),
        FRAME_VERSION as usize
    );
    assert_eq!(ts_string(&source, "frame.ts", "SUBPROTOCOL"), SUBPROTOCOL);
    assert_eq!(
        ts_string(&source, "frame.ts", "SUBPROTOCOL_V1"),
        SUBPROTOCOL_V1
    );
}
