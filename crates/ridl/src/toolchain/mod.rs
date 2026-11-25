//! Install and select immutable versioned toolchains.
mod install;
mod paths;
mod selection;
pub use install::install;
pub use paths::{release_target, toolchain_root};
pub use selection::*;
pub const RELEASE_BASE_URL: &str = "https://github.com/driftsys/ridl/releases/download";
