//! Shared input and result types for workspace tools.
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct OverlayInput {
    /// The .typl, .ridl, .rsdl or .rxdl file path whose parent directory exists, relative to the server's working directory unless absolute. A .rxdl overlay draws the warning RIDL-417 and is not compiled.
    pub path: String,
    /// The full unsaved text of the source file.
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
