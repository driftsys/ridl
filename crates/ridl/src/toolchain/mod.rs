//! Install and select immutable versioned toolchains.
mod install;
mod paths;
mod selection;
mod uninstall;
pub use install::install;
pub use paths::{process_toolchain_root, release_target, toolchain_root};
pub use selection::*;
pub use uninstall::uninstall;
pub const RELEASE_BASE_URL: &str = "https://github.com/driftsys/ridl/releases/download";
