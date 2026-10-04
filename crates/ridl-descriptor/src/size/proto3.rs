//! proto3 states — a placeholder until Task 6 of the catalog descriptor plan
//! (driftsys/ridl#380) replaces this body. It panics when reached, so a
//! routing error in `size_state` fails a test instead of reading as an
//! absent row.

use super::{Ctx, SizeState};

pub(crate) fn state(_type_name: &str, _ctx: &Ctx<'_>) -> Option<SizeState> {
    unimplemented!(
        "proto3 size states are Task 6 of the catalog descriptor plan (driftsys/ridl#380)"
    )
}
