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
/// A placeholder that fails every download. The release workflow refuses to
/// sign while the marker in this value is present, and the value must be
/// replaced with the release minisign public key before the change is merged.
pub const RELEASE_PUBLIC_KEY: &str = "RIDL-RELEASE-KEY-PLACEHOLDER: the owner replaces this with the release minisign public key before merge";
