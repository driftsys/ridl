//! FlatBuffers states — Task 7 replaces this body.

use super::{Ctx, SizeState};

pub(crate) fn state(type_name: &str, ctx: &Ctx<'_>) -> Option<SizeState> {
    let _ = (type_name, ctx);
    None
}
