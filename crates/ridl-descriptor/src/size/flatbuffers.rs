//! FlatBuffers states — a placeholder until Task 7 of the catalog descriptor
//! plan replaces this body. It panics when reached, so a routing error in
//! `size_state` fails a test instead of reading as an absent row.

use super::{Ctx, SizeState};

pub(crate) fn state(_type_name: &str, _ctx: &Ctx<'_>) -> Option<SizeState> {
    unimplemented!("FlatBuffers size states are Task 7 of the catalog descriptor plan")
}
