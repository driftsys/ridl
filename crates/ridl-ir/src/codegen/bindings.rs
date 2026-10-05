//! The transport bindings the toolchain knows, and the per-message overhead
//! each one adds.
//!
//! A plugin that sizes a socket message adds the overhead of the binding the
//! message crosses, so the deployment section carries this table
//! (`docs/design/codegen-plugins.md`, section "The deployment section"). A row
//! is stated here once and rendered into every section.

use super::v1;

/// One known transport binding.
pub struct Known {
    /// The binding's name, for example `"websocket"`.
    pub name: &'static str,
    /// The version of the binding document the overheads come from.
    pub version: &'static str,
    /// The largest frame header the binding adds to a message, in bytes.
    /// `None` when the binding states no frame header.
    pub frame_header_max_bytes: Option<u32>,
    /// The bytes the binding's envelope adds to a message. `None` when the
    /// binding states no envelope.
    pub envelope_bytes: Option<u32>,
}

/// Every known binding. The table is empty because no binding document states
/// a frame layout yet (driftsys/ridl#265); adding a row here adds it to every
/// deployment section.
pub const KNOWN: &[Known] = &[];

/// The known bindings as deployment-section messages, in name order.
pub fn bindings() -> Vec<v1::Binding> {
    render(KNOWN)
}

/// Renders `table` as deployment-section messages, in name order.
pub(super) fn render(table: &[Known]) -> Vec<v1::Binding> {
    let mut rows: Vec<&Known> = table.iter().collect();
    rows.sort_by_key(|row| row.name);
    rows.into_iter()
        .map(|row| v1::Binding {
            name: row.name.to_string(),
            version: row.version.to_string(),
            frame_header_max_bytes: row.frame_header_max_bytes,
            envelope_bytes: row.envelope_bytes,
        })
        .collect()
}
