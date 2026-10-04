//! proto3 states — Task 6 (driftsys/ridl#380) replaces this body.

use super::{Ctx, SizeState};

pub(crate) fn state(type_name: &str, ctx: &Ctx<'_>) -> Option<SizeState> {
    let _ = (type_name, ctx);
    None
}
