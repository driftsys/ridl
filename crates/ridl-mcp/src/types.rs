//! Shared input and result types for workspace tools.
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct OverlayInput {
    /// The source file path or root ridl.toml path whose parent directory exists, relative to the server's working directory unless absolute. Source paths must end in .typl, .ridl, .rsdl, .rxdl or .rmdl. Other manifest overlays are unsupported. A .rxdl or .rmdl overlay in a package directory draws the warning RIDL-417 and is not compiled.
    pub path: String,
    /// The full unsaved text of the source file or root manifest.
    pub source: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct Location {
    pub path: String,
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct WorkspaceStatus {
    pub root: String,
    pub errors: usize,
    pub warnings: usize,
    pub notes: Vec<String>,
}
